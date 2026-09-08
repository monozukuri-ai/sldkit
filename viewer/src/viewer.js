import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

const $ = (id) => document.getElementById(id);
const data = JSON.parse($('sldkit-scene').textContent);
const number = (value) => value.toLocaleString('en-US');
const colors = ['#65998e', '#819db4', '#bd996f', '#9e8cab', '#8f9e6b', '#b88179'];
const text = (id, value) => { $(id).textContent = value; };
text('file-title', data.title);
text('status', `Geometry: ${data.geometry_status.replaceAll('_', ' ')}`);
text('mesh-count', number(data.counts.meshes));
text('triangle-count', number(data.counts.triangles));
text('geometry-summary', `${number(data.counts.vertices)} vertices · ${data.body_count} parsed bodies · ${data.face_count} parsed faces`);
text('preview-count', `(${data.previews.length})`);
text('unit', data.unit || 'Unit unknown');

function diagnostic(code, message) {
  const item = document.createElement('div');
  item.className = 'diagnostic';
  const label = document.createElement('code');
  label.textContent = code;
  const description = document.createElement('p');
  description.textContent = message;
  item.append(label, description);
  $('diagnostic-list').append(item);
}
if (data.parse_status) diagnostic('Document parse', data.parse_status);
diagnostic('Geometry decode', data.geometry_status);
for (const entry of [...data.diagnostics, ...data.losses]) diagnostic(entry.code, entry.message);
for (const warning of data.warnings) diagnostic('Viewer', warning);
text('diagnostic-count', `(${data.diagnostics.length + data.losses.length + data.warnings.length})`);

function documentItem(name, lines) {
  if (!lines.length) return;
  const container = document.createElement('div');
  container.className = 'doc-item';
  const label = document.createElement('strong');
  label.textContent = name;
  const list = document.createElement('ul');
  for (const line of lines) {
    const item = document.createElement('li');
    item.textContent = line;
    list.append(item);
  }
  container.append(label, list);
  $('document-info').append(container);
}
if (data.document) {
  $('document-section').hidden = false;
  documentItem('Type', [data.document.kind]);
  documentItem('Configurations', data.document.configurations.map((c) => c ?? 'Unnamed'));
  documentItem('References', data.document.references.map((r) =>
    [r.name, r.path, r.configuration].filter(Boolean).join(' · ') || 'Unresolved reference'));
  for (const sheet of data.document.sheets) {
    documentItem(`Sheet: ${sheet.name ?? 'Unnamed'}`, sheet.views.length
      ? sheet.views.map((v) => [v.name, v.document].filter(Boolean).join(' · ') || 'Unnamed view')
      : ['No decoded views']);
  }
}

function tab(preview) {
  $('panel-3d').hidden = preview;
  $('panel-preview').hidden = !preview;
  $('tab-3d').setAttribute('aria-pressed', String(!preview));
  $('tab-preview').setAttribute('aria-pressed', String(preview));
}
$('tab-3d').onclick = () => tab(false);
$('tab-preview').onclick = () => tab(true);
$('empty-preview').onclick = () => tab(true);
$('empty-preview').hidden = data.previews.length === 0;
for (let i = 0; i < data.previews.length; i++) {
  $('preview-select').add(new Option(data.previews[i].name, String(i)));
}
function preview() {
  const entry = data.previews[Number($('preview-select').value)];
  $('preview-image').hidden = !entry;
  $('preview-empty').hidden = !!entry;
  if (entry) {
    $('preview-image').src = entry.url;
    $('preview-image').alt = `${entry.name} — saved in the source file`;
  }
}
$('preview-image').onerror = () => {
  $('preview-image').hidden = true;
  $('preview-empty').hidden = false;
  text('preview-empty', 'The browser could not decode this saved image.');
};
$('preview-select').onchange = preview;
$('preview-select').disabled = !data.previews.length;
preview();

function empty(title, message) {
  $('empty').hidden = false;
  text('empty-title', title);
  text('empty-message', message);
}

function startRenderer() {
  const viewport = $('viewport');
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.setClearColor(0xffffff, 0);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  viewport.prepend(renderer.domElement);
  renderer.domElement.setAttribute('aria-label', 'Parsed triangle meshes');
  renderer.domElement.addEventListener('webglcontextlost', (event) => {
    event.preventDefault();
    empty('3D context lost', 'Reload this HTML to retry. Saved previews and analysis details remain available.');
  });
  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight(0xffffff, 0x718378, 2.5));
  const light = new THREE.DirectionalLight(0xffffff, 2.2);
  light.position.set(3, 5, 4);
  scene.add(light);
  const fill = new THREE.DirectionalLight(0xffffff, 0.8);
  fill.position.set(-3, 1, -4);
  scene.add(fill);
  const camera = new THREE.PerspectiveCamera(35, 1, 0.01, 10000);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = false;
  controls.screenSpacePanning = true;

  // Preserve source coordinates in the payload; rebase only GPU display buffers.
  const bounds = new THREE.Box3();
  const vertex = new THREE.Vector3();
  for (const mesh of data.meshes) for (const p of mesh.vertices) bounds.expandByPoint(vertex.fromArray(p));
  const origin = bounds.getCenter(new THREE.Vector3());
  const span = bounds.getSize(new THREE.Vector3()).length() || 1;
  const axes = new THREE.AxesHelper(span * 0.23);
  scene.add(axes);
  const objects = [];
  const validBodyIds = new Set(data.body_ids);
  const ownershipResolved = data.meshes.every((m) => m.body_id && validBodyIds.has(m.body_id));
  for (const c of data.configurations) {
    const option = new Option(c.name ?? `Configuration ${c.ordinal}`, c.id);
    option.disabled = !ownershipResolved || c.body_ids === null;
    $('configuration').add(option);
  }
  $('configuration').disabled = !ownershipResolved || !data.configurations.some((c) => c.body_ids !== null);
  text('ownership-note', ownershipResolved
    ? 'Configuration filtering uses resolved mesh body references.'
    : 'Mesh ownership is unresolved. Showing collected meshes without configuration filtering.');

  for (const [index, mesh] of data.meshes.entries()) {
    const group = new THREE.Group();
    const positions = new Float32Array(mesh.vertices.length * 3);
    for (let i = 0; i < mesh.vertices.length; i++) {
      positions[i * 3] = mesh.vertices[i][0] - origin.x;
      positions[i * 3 + 1] = mesh.vertices[i][1] - origin.y;
      positions[i * 3 + 2] = mesh.vertices[i][2] - origin.z;
    }
    let geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(mesh.triangles.flat());
    if (mesh.corner_normals.length) {
      const indexed = geometry;
      geometry = indexed.toNonIndexed();
      indexed.dispose();
      geometry.setAttribute('normal', new THREE.Float32BufferAttribute(mesh.corner_normals.flat(), 3));
    } else if (mesh.normals.length) {
      geometry.setAttribute('normal', new THREE.Float32BufferAttribute(mesh.normals.flat(), 3));
    }
    const color = mesh.color ? new THREE.Color().setRGB(...mesh.color.slice(0, 3)) : new THREE.Color(colors[index % colors.length]);
    const material = new THREE.MeshStandardMaterial({
      color, roughness: 0.7, metalness: 0.05, side: THREE.DoubleSide,
      flatShading: !mesh.normals.length && !mesh.corner_normals.length,
      opacity: mesh.color?.[3] ?? 1, transparent: (mesh.color?.[3] ?? 1) < 1,
      polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1,
    });
    // Missing normals use flat display shading; no source fields are rewritten.
    const solid = new THREE.Mesh(geometry, material);
    group.add(solid);
    let edges = null;
    if (mesh.feature_edges.length) {
      const lines = new THREE.BufferGeometry();
      lines.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      lines.setIndex(mesh.feature_edges.flat());
      edges = new THREE.LineSegments(lines, new THREE.LineBasicMaterial({ color: 0x334d46 }));
      group.add(edges);
    }
    scene.add(group);
    const row = document.createElement('label');
    row.className = 'mesh-row';
    row.title = `${mesh.id}\nBody: ${mesh.body_id ?? 'unresolved'}\nFaces: ${mesh.face_ids.join(', ') || 'unresolved'}\nExactness: ${mesh.exactness}`;
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = mesh.visible !== false;
    const swatch = document.createElement('i');
    swatch.className = 'swatch';
    swatch.style.background = `#${color.getHexString()}`;
    const name = document.createElement('span');
    name.textContent = mesh.name || `Mesh ${index + 1}`;
    const count = document.createElement('small');
    count.textContent = number(mesh.triangles.length);
    row.append(checkbox, swatch, name, count);
    $('mesh-list').append(row);
    objects.push({ group, material, edges, checkbox, row, mesh });
    checkbox.onchange = visibility;
  }

  function render() { renderer.render(scene, camera); }
  function visibility() {
    const config = data.configurations.find((c) => c.id === $('configuration').value);
    let visible = 0;
    for (const item of objects) {
      const belongs = !config || config.body_ids.includes(item.mesh.body_id);
      item.group.visible = item.checkbox.checked && belongs;
      item.row.classList.toggle('filtered', !belongs);
      if (item.group.visible) visible++;
    }
    text('visible-count', `${visible} / ${objects.length} meshes visible`);
    if (visible) $('empty').hidden = true;
    else empty('No visible meshes', 'Enable a mesh or choose a different configuration.');
    render();
  }
  function fit() {
    const box = new THREE.Box3();
    for (const item of objects) if (item.group.visible) box.expandByObject(item.group);
    if (box.isEmpty()) return;
    const center = box.getCenter(new THREE.Vector3());
    const radius = Math.max(box.getSize(new THREE.Vector3()).length() / 2, span * 1e-6);
    const view = $('view').value;
    const direction = new THREE.Vector3(...({ iso: [1, 0.8, 1], front: [0, 0, 1], top: [0, 1, 0], right: [1, 0, 0] }[view]));
    camera.up.set(0, view === 'top' ? 0 : 1, view === 'top' ? -1 : 0);
    const vertical = THREE.MathUtils.degToRad(camera.fov / 2);
    const angle = Math.min(vertical, Math.atan(Math.tan(vertical) * camera.aspect));
    const distance = radius / Math.sin(angle) * 1.2;
    camera.position.copy(center).addScaledVector(direction.normalize(), distance);
    camera.near = Math.max(distance / 10000, span * 1e-8);
    camera.far = Math.max(distance * 100, span * 100);
    camera.updateProjectionMatrix();
    controls.target.copy(center);
    controls.minDistance = radius * 0.01;
    controls.maxDistance = Math.max(distance * 30, span * 30);
    controls.update();
    render();
  }
  $('fit').onclick = fit;
  $('view').onchange = fit;
  $('configuration').onchange = () => { visibility(); fit(); };
  $('show-all').onclick = () => {
    $('configuration').value = '';
    for (const item of objects) item.checkbox.checked = true;
    visibility(); fit();
  };
  $('style').onchange = () => {
    for (const item of objects) item.material.wireframe = $('style').value === 'wireframe';
    render();
  };
  $('feature-edges').disabled = objects.every((o) => !o.edges);
  $('feature-edges').onchange = () => {
    for (const item of objects) if (item.edges) item.edges.visible = $('feature-edges').checked;
    render();
  };
  $('axes').onchange = () => { axes.visible = $('axes').checked; $('axis-key').hidden = !axes.visible; render(); };
  controls.addEventListener('change', render);
  const observer = new ResizeObserver(() => {
    const { width, height } = viewport.getBoundingClientRect();
    if (width <= 0 || height <= 0) return;
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
    render();
  });
  observer.observe(viewport);
  const { width, height } = viewport.getBoundingClientRect();
  camera.aspect = width / height;
  renderer.setSize(width, height);
  visibility(); fit();
  document.documentElement.dataset.renderer = 'ready';
}

if (!data.meshes.length) {
  $('mesh-section').hidden = true;
  $('toolbar').hidden = true;
  $('axis-key').hidden = true;
  empty('No mesh available', 'The parser did not return a displayable triangle mesh. Saved previews and analysis details may still be available.');
  if (data.previews.length) tab(true);
  document.documentElement.dataset.renderer = 'empty';
} else {
  try { startRenderer(); }
  catch (error) {
    empty('3D display unavailable', 'A WebGL2-capable browser is required. Saved previews and analysis details remain available.');
    diagnostic('Viewer rendering', String(error.message));
    for (const control of $('toolbar').querySelectorAll('button, select, input')) control.disabled = true;
    document.documentElement.dataset.renderer = 'unavailable';
  }
}
document.documentElement.dataset.viewer = 'ready';
