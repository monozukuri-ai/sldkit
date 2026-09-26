# パーサ改善の進捗 — 2026-09-26

機能不足の調査で提案した順序に沿って、Partの読み取り修正、境界API、
Assembly・Drawingの検証準備、回帰検証を進めた。
この記録はローカルの変更・実行結果であり、リリース済み機能の一覧ではない。

## 1. TEST1 / TEST2のB-Rep復元

頂点を持たない円形のring edgeを、通常のFINと同じ「頂点が必須」という
条件で拒否していた。既存の限定schema `SCH_3701229_37102_13006` に対して、
相互参照・向き・単一FINループ・正確な円carrierを検証するadapterを追加した。
通常のFINは引き続き共有`parasolid-core`で検証する。
不正な参照や未対応curveは、B-Repを出さない従来の挙動を維持する。

| 入力 | 修正前のbody / face | 修正後のbody / face | 現在の状態 |
|---|---:|---:|---|
| TEST1.SLDPRT | 0 / 0 | 1 / 9 | `decoded` |
| TEST2.SLDPRT | 0 / 0 | 1 / 4 | `partial`：保存meshのface所属に未解決あり |
| M5-Controlled.SLDPRT | 4 / 19 | 4 / 19 | `decoded`、既存の検証条件を維持 |

円周・周期面をendpoint形式で表すための継ぎ目の頂点・辺は、既存の
`derived_closed_circle_seam` / `derived_periodic_seam` として区別する。
元ファイルの頂点を追加したと扱わない。TEST1/TEST2についてSolidWorks APIの
独立採取値はまだない。保存差分の一般的な置換・削除・hierarchy更新も未対応。

## 2. 曲線の有効範囲

`GeometryEdge.derived_parameter_interval`をRust/JSON/Pythonに追加した。
既存の`parameter_range`を変更せず、導出方法・許容誤差・端点誤差を保持する。
Pythonでは`effective_parameter_range`から既存値を優先して取得できる。

- 直線：非単位方向ベクトルも含む端点の射影。
- NURBS：clamped・非周期・支持範囲全体と端点が一意に一致する限定ケース。
- off-curve、曖昧な端点、部分的NURBS trim等は導出しない。
- 穴などの複数loopを持つ面では、diskを前提とする`V-E+F`値を公開しない。

M5では直線38辺とNURBS 2辺に導出範囲が付いた。既存のSTEP oracleとの
比較で、支持形状・導出境界・向きの各gateが合格した。
保存されたsource trim range、outer/inner loop、一般的な穴・seam・周期面の
完全な再構築は未達であり、`source_trim_gate_passed=false`を維持している。

詳細な契約は[geometry.md](geometry.md#endpoint-derived-edge-intervals)。

## 3. Assembly配置

既存XMLに`swTransform`数値列があることを確認したが、並び・単位・方向・
親/root座標の対応を認定できる独立データがない。
そのため配置APIの公開には進まず、[採取ツール](../scripts/capture_assembly_transforms.ps1)
と[採取・受入手順](assembly-validation.md)を追加した。

ツールは`Transform2.ArrayData`と、SDK自身で変換した原点・3本の基準点を
保存する。配置の回転・移動・nested assemblyを、native数値列と独立に照合
するためのもの。PowerShellの構文・mock検証は合格したが、Windows/SolidWorks
での実行は未実施。ユーザー確認でも採取済みデータはない。

## 4. Drawing意味解析

既存の採取・差分ツールを使う[検証順序](drawing-validation.md#next-decoding-sequence)
を具体化した。単一viewのX/Y移動、scale、rotation、投影view追加、note追加の
順に、一操作ずつnative/APIを比較する。
binary recordの境界・所属・field対応は未認定のため、配置・寸法・注記の
runtime解析は今回追加していない。

## 5. 公開APIと回帰検証

導出範囲の型・旧JSON互換性・誤った範囲や向きの拒否を検証した。
CIにRust workspace全体、Python全体、OCPの独立形状検証、fuzz入口のcompile、
採取ツールのPowerShell mock検証を追加した。
既存fuzz targetはgeometryとDrawing structureの入口も呼ぶようにした。
feature/sketch/PMIの公開モデルは引き続き未実装。

| 検証 | 今回の結果 |
|---|---|
| Rust workspace | 63 passed |
| patched backend | 1,294 passed |
| Python（OCP 7.9.3.1あり） | 169 passed、skipなし |
| workspace / backend Clippy、fmt、Ruff | 合格 |
| geometry corpus：上記3 Part | 合格：Rust/Python JSON一致、決定性、参照、byte partition等 |
| M5 controlled oracle | evidence / native geometry / exact byte partitionすべて合格 |
| M5 trim比較 | 支持形状・導出境界・向きは合格、source trimは未達 |
| Assembly採取ツール | PowerShell 7.4.15で構文・mock合格、COM未実行 |
| fuzz | compile合格、fuzz campaignは未実施 |
| CI | workflow変更のみ。remote CI未実行 |

次の主要な依存物は、Assemblyの移動・回転・nested caseとDrawingの一操作差分の
native/API対である。その前に残るPart側の作業はsource trimの解読と、
穴・seam・周期面の独立oracle付きfixture拡張。取得できない値を推定して
対応済みにはしていない。
