//! Typed document unit settings derived from `SW_Unit*` system properties.
//!
//! The integer codes are exact source values. Their names apply the named
//! `SolidWorks` API enumerations and are therefore labelled as inferred.

use sldkit_core::{
    CustomProperty, Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentUnits, PropertyKind,
    SourceValue, UnitSetting, ValueOrigin,
};

struct Enumeration {
    name: &'static str,
    members: &'static [(i64, &'static str)],
}

/// `swUnitSystem_e`.
const UNIT_SYSTEM: Enumeration = Enumeration {
    name: "swUnitSystem_e",
    members: &[
        (1, "cgs"),
        (2, "mks"),
        (3, "ips"),
        (4, "mmgs"),
        (5, "custom"),
    ],
};

/// `swLengthUnit_e`.
const LENGTH_UNIT: Enumeration = Enumeration {
    name: "swLengthUnit_e",
    members: &[
        (0, "millimeter"),
        (1, "centimeter"),
        (2, "meter"),
        (3, "inch"),
        (4, "foot"),
        (5, "feet_inches"),
        (6, "angstrom"),
        (7, "nanometer"),
        (8, "micron"),
        (9, "mil"),
        (10, "microinch"),
    ],
};

/// `swAngleUnit_e`.
const ANGLE_UNIT: Enumeration = Enumeration {
    name: "swAngleUnit_e",
    members: &[
        (0, "degrees"),
        (1, "degrees_minutes"),
        (2, "degrees_minutes_seconds"),
        (3, "radians"),
    ],
};

/// `swUnitsMassPropMass_e`.
const MASS_UNIT: Enumeration = Enumeration {
    name: "swUnitsMassPropMass_e",
    members: &[(0, "milligram"), (1, "gram"), (2, "kilogram"), (3, "pound")],
};

/// `swUnitsMassPropVolume_e`.
const VOLUME_UNIT: Enumeration = Enumeration {
    name: "swUnitsMassPropVolume_e",
    members: &[
        (0, "cubic_angstrom"),
        (1, "cubic_nanometer"),
        (2, "cubic_micron"),
        (3, "cubic_millimeter"),
        (4, "cubic_centimeter"),
        (5, "cubic_meter"),
        (6, "cubic_microinch"),
        (7, "cubic_mil"),
        (8, "cubic_inch"),
        (9, "cubic_foot"),
    ],
};

/// Derive typed unit settings from global system properties.
///
/// Returns `None` when no unit property is present. Conflicting values for
/// one property name are withheld with a diagnostic rather than chosen.
pub(crate) fn derive_document_units(
    properties: &[CustomProperty],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DocumentUnits> {
    let mut units = DocumentUnits::default();
    let mut any = false;
    let mut setting = |name: &str, enumeration: &Enumeration| {
        let value = unit_setting(properties, name, enumeration, diagnostics);
        any |= value.is_some();
        value
    };
    units.unit_system = setting("SW_UnitSystem", &UNIT_SYSTEM);
    units.length = setting("SW_UnitsLinear", &LENGTH_UNIT);
    units.dual_length = setting("SW_UnitsDualLinear", &LENGTH_UNIT);
    units.angle = setting("SW_UnitsAngular", &ANGLE_UNIT);
    units.mass_property_length = setting("SW_UnitsMassPropLength", &LENGTH_UNIT);
    units.mass = setting("SW_UnitsMassPropMass", &MASS_UNIT);
    units.volume = setting("SW_UnitsMassPropVolume", &VOLUME_UNIT);
    let mut places = |name: &str| {
        let value = integer_property(properties, name, diagnostics);
        any |= value.is_some();
        value
    };
    units.length_decimal_places = places("SW_UnitsLinearDecimalPlaces");
    units.dual_length_decimal_places = places("SW_UnitsDualLinearDecimalPlaces");
    units.angle_decimal_places = places("SW_UnitsAngularDecimalPlaces");
    units.mass_property_decimal_places = places("SW_UnitsMassPropDecimalPlaces");
    any.then_some(units)
}

fn unit_setting(
    properties: &[CustomProperty],
    name: &str,
    enumeration: &Enumeration,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<UnitSetting> {
    let code = integer_property(properties, name, diagnostics)?;
    let member = enumeration
        .members
        .iter()
        .find(|(value, _)| *value == code.value)
        .map(|(_, member)| *member);
    let enumeration_evidence = format!("enumeration.{}", enumeration.name);
    let unit_name = member.map(|member| {
        let mut evidence = code.evidence.clone();
        evidence.push(enumeration_evidence.clone());
        SourceValue::new(member.to_owned(), ValueOrigin::Inferred, evidence)
    });
    if unit_name.is_none() {
        diagnostics.push(
            Diagnostic::new(
                "units.code_unrecognized",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "unit setting code is outside the known enumeration members",
            )
            .with_detail("property", name)
            .with_detail("enumeration", enumeration.name)
            .with_detail("code", code.value.to_string()),
        );
    }
    Some(UnitSetting {
        property_name: name.to_owned(),
        code,
        name: unit_name,
    })
}

fn integer_property(
    properties: &[CustomProperty],
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SourceValue<i64>> {
    let mut values: Vec<SourceValue<i64>> = Vec::new();
    for property in properties
        .iter()
        .filter(|property| property.kind == PropertyKind::System && property.name.value == name)
    {
        let Some(raw) = property.raw_value.as_ref() else {
            continue;
        };
        let Ok(parsed) = raw.value.trim().parse::<i64>() else {
            diagnostics.push(
                Diagnostic::new(
                    "units.value_malformed",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "unit setting value is not a signed decimal integer",
                )
                .in_stream(&property.stream_path)
                .with_detail("property", name)
                .with_detail("raw_value", &raw.value),
            );
            continue;
        };
        if let Some(existing) = values.iter_mut().find(|value| value.value == parsed) {
            existing.evidence.extend(raw.evidence.iter().cloned());
            existing.evidence.sort();
            existing.evidence.dedup();
        } else {
            values.push(SourceValue::new(
                parsed,
                ValueOrigin::Source,
                raw.evidence.clone(),
            ));
        }
    }
    if values.len() > 1 {
        diagnostics.push(
            Diagnostic::new(
                "units.value_conflict",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Preserved,
                "unit setting has multiple distinct source values; none is selected",
            )
            .with_detail("property", name)
            .with_detail(
                "values",
                values
                    .iter()
                    .map(|value| value.value.to_string())
                    .collect::<Vec<_>>()
                    .join("|"),
            ),
        );
        return None;
    }
    values.into_iter().next()
}

#[cfg(test)]
mod tests {
    use sldkit_core::{
        CustomProperty, PropertyKind, PropertyScope, PropertyValueState, SourceValue, ValueOrigin,
    };

    use super::derive_document_units;

    fn property(name: &str, value: &str, kind: PropertyKind) -> CustomProperty {
        CustomProperty {
            name: SourceValue::new(name.to_owned(), ValueOrigin::Source, Vec::new()),
            raw_value: Some(SourceValue::new(
                value.to_owned(),
                ValueOrigin::Source,
                vec![format!("entry#property:{name}:value")],
            )),
            value_type: Some("i4".to_owned()),
            value_state: PropertyValueState::Present,
            kind,
            scope: PropertyScope::Global,
            configuration: None,
            configuration_index: None,
            stream_path: "docProps/ISolidWorksInformation.xml".to_owned(),
            pid: None,
        }
    }

    #[test]
    fn codes_are_source_values_and_names_are_inferred() -> Result<(), &'static str> {
        let properties = vec![
            property("SW_UnitSystem", "4", PropertyKind::System),
            property("SW_UnitsLinear", "0", PropertyKind::System),
            property("SW_UnitsLinearDecimalPlaces", "2", PropertyKind::System),
            property("SW_UnitsAngular", "99", PropertyKind::System),
            property("SW_UnitsMassPropMass", "2", PropertyKind::Custom),
        ];
        let mut diagnostics = Vec::new();
        let units = derive_document_units(&properties, &mut diagnostics).ok_or("units missing")?;
        let system = units
            .unit_system
            .as_ref()
            .map(|u| (u.code.value, u.name.as_ref().map(|n| n.value.clone())));
        assert_eq!(system, Some((4, Some("mmgs".to_owned()))));
        let length = units
            .length
            .as_ref()
            .map(|u| u.name.as_ref().map(|n| n.origin));
        assert_eq!(length, Some(Some(ValueOrigin::Inferred)));
        assert_eq!(
            units.length_decimal_places.as_ref().map(|v| v.value),
            Some(2)
        );
        let angle = units
            .angle
            .as_ref()
            .map(|u| (u.code.value, u.name.is_none()));
        assert_eq!(angle, Some((99, true)));
        assert!(
            units.mass.is_none(),
            "custom properties are not unit settings"
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "units.code_unrecognized")
        );
        Ok(())
    }

    #[test]
    fn conflicting_values_are_withheld_and_absence_is_none() {
        let properties = vec![
            property("SW_UnitsLinear", "0", PropertyKind::System),
            property("SW_UnitsLinear", "3", PropertyKind::System),
        ];
        let mut diagnostics = Vec::new();
        let units = derive_document_units(&properties, &mut diagnostics);
        assert!(units.is_none());
        assert!(diagnostics.iter().any(|d| d.code == "units.value_conflict"));
        assert!(derive_document_units(&[], &mut Vec::new()).is_none());
    }
}
