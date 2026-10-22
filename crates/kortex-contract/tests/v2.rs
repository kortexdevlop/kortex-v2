use kortex_contract::v2::Contract;

fn json_from_toml(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(value) => serde_json::Value::String(value.clone()),
        toml::Value::Integer(value) => serde_json::json!(value),
        toml::Value::Float(value) => serde_json::json!(value),
        toml::Value::Boolean(value) => serde_json::json!(value),
        toml::Value::Datetime(value) => serde_json::Value::String(value.to_string()),
        toml::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(json_from_toml).collect())
        }
        toml::Value::Table(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), json_from_toml(value)))
                .collect(),
        ),
    }
}

#[test]
fn every_existing_tool_adapts_without_losing_its_run_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools");
    let mut count = 0;
    for universe in std::fs::read_dir(root).unwrap().flatten() {
        if !universe.file_type().unwrap().is_dir() {
            continue;
        }
        for tool in std::fs::read_dir(universe.path()).unwrap().flatten() {
            let path = tool.path().join("tool.toml");
            if !path.is_file() {
                continue;
            }
            let legacy = kortex_contract::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
            let v2 = Contract::from_v1(&legacy).unwrap();
            assert_eq!(v2.search, legacy.search);
            assert!(
                v2.validate().is_empty(),
                "{}: {:?}",
                legacy.tool.id,
                v2.validate()
            );
            let run = v2.operation("run").unwrap();
            assert_eq!(run.inputs, legacy.inputs);
            assert_eq!(run.outputs, legacy.outputs);
            assert_eq!(run.params.len(), legacy.params.len());
            for (v2_param, legacy_param) in run.params.iter().zip(&legacy.params) {
                assert_eq!(v2_param.id, legacy_param.id);
                assert_eq!(v2_param.ty, legacy_param.ty);
                assert_eq!(
                    v2_param.default,
                    legacy_param.default.as_ref().map(json_from_toml)
                );
            }
            assert_eq!(v2.variants.len(), legacy.variants.len());
            for (v2_variant, legacy_variant) in v2.variants.iter().zip(&legacy.variants) {
                assert_eq!(v2_variant.id, legacy_variant.id);
                assert_eq!(
                    v2_variant.preset,
                    legacy_variant
                        .preset
                        .as_ref()
                        .map(|table| json_from_toml(&toml::Value::Table(table.clone())))
                );
            }
            assert_eq!(run.permissions, legacy.permissions);
            assert_eq!(run.limits, legacy.limits);
            let wire = serde_json::to_string(&v2).unwrap();
            let roundtrip = serde_json::from_str::<Contract>(&wire).unwrap();
            assert_eq!(roundtrip, v2);
            assert!(!wire.contains("$serde_json::private::Number"));
            count += 1;
        }
    }
    assert_eq!(count, 44);
}

#[test]
fn rejects_unsupported_schema_duplicate_operations_and_inconsistent_capabilities() {
    let legacy =
        kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml")).unwrap();
    let mut contract = Contract::from_v1(&legacy).unwrap();
    contract.schema = 999;
    contract.operations[0].execution.max_memory_bytes = Some(0);
    contract.operations.push(contract.operations[0].clone());
    assert!(contract.validate().len() >= 3);
}
