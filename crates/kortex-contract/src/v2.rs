//! Versioned operation descriptions. These are not a new executable engine ABI.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Input, Issue, Limits, Output, Param, ParamLevel, Permissions, Search, ToolContract, ToolMeta,
    Variant,
};

pub const SCHEMA: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema: u32,
    pub tool: ToolMeta,
    pub search: Search,
    pub variants: Vec<ContractVariant>,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub id: String,
    pub inputs: Vec<Input>,
    pub params: Vec<OperationParam>,
    pub outputs: Vec<Output>,
    /// Additional ports carry handles, never file contents or filesystem paths.
    pub references: Vec<ReferencePort>,
    pub permissions: Permissions,
    pub limits: Limits,
    pub effects: Effects,
    pub execution: Execution,
}

/// A parameter encoded with JSON values at the public v2 contract boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationParam {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: crate::PortType,
    pub level: Option<ParamLevel>,
    pub default: Option<Value>,
    pub range: Option<Vec<f64>>,
    pub options: Option<Vec<String>>,
    pub label: Option<crate::Localized>,
}

/// A variant encoded with JSON values at the public v2 contract boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractVariant {
    pub id: String,
    pub slug: crate::Localized,
    pub name: crate::Localized,
    pub preset: Option<Value>,
}

fn toml_json(value: &toml::Value) -> Result<Value, Issue> {
    Ok(match value {
        toml::Value::String(value) => Value::String(value.clone()),
        toml::Value::Integer(value) => Value::Number((*value).into()),
        toml::Value::Float(value) => serde_json::Number::from_f64(*value)
            .map(Value::Number)
            .ok_or_else(|| Issue("valeur décimale non finie dans le contrat".into()))?,
        toml::Value::Boolean(value) => Value::Bool(*value),
        toml::Value::Datetime(value) => Value::String(value.to_string()),
        toml::Value::Array(values) => {
            Value::Array(values.iter().map(toml_json).collect::<Result<_, _>>()?)
        }
        toml::Value::Table(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), toml_json(value)?)))
                .collect::<Result<_, Issue>>()?,
        ),
    })
}

fn json_toml(value: &Value) -> Result<toml::Value, Issue> {
    Ok(match value {
        Value::Null => return Err(Issue("valeur nulle invalide dans un paramètre".into())),
        Value::Bool(value) => toml::Value::Boolean(*value),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                toml::Value::Integer(value)
            } else if let Some(value) = value.as_f64() {
                toml::Value::Float(value)
            } else {
                return Err(Issue("valeur numérique invalide dans un paramètre".into()));
            }
        }
        Value::String(value) => toml::Value::String(value.clone()),
        Value::Array(values) => {
            toml::Value::Array(values.iter().map(json_toml).collect::<Result<_, _>>()?)
        }
        Value::Object(values) => toml::Value::Table(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), json_toml(value)?)))
                .collect::<Result<_, Issue>>()?,
        ),
    })
}

impl TryFrom<&Param> for OperationParam {
    type Error = Issue;

    fn try_from(value: &Param) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id.clone(),
            ty: value.ty.clone(),
            level: value.level,
            default: value.default.as_ref().map(toml_json).transpose()?,
            range: value.range.clone(),
            options: value.options.clone(),
            label: value.label.clone(),
        })
    }
}

impl TryFrom<&Variant> for ContractVariant {
    type Error = Issue;

    fn try_from(value: &Variant) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id.clone(),
            slug: value.slug.clone(),
            name: value.name.clone(),
            preset: value
                .preset
                .as_ref()
                .map(|table| toml_json(&toml::Value::Table(table.clone())))
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencePort {
    pub id: String,
    pub kind: ReferenceKind,
    pub direction: Direction,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferenceKind {
    Resource { mime: String },
    Project { format: String, schema: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub mutates_project: bool,
    pub writes_resources: bool,
    /// A host must not infer retry safety from a function's name.
    pub retry_safe: bool,
    pub undoable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub background: bool,
    pub cancellable: bool,
    pub resumable: bool,
    pub max_memory_bytes: Option<u64>,
}

impl Contract {
    /// Preserve every v1 port, parameter, permission and limit as a single `run`.
    /// Legacy engines do not acquire cancellation/retry/undo guarantees by migration.
    pub fn from_v1(legacy: &ToolContract) -> Result<Self, Vec<Issue>> {
        let issues = crate::validate(legacy, std::slice::from_ref(&legacy.tool.universe));
        if !issues.is_empty() {
            return Err(issues);
        }
        Ok(Self {
            schema: SCHEMA,
            tool: legacy.tool.clone(),
            search: legacy.search.clone(),
            variants: legacy
                .variants
                .iter()
                .map(ContractVariant::try_from)
                .collect::<Result<_, _>>()
                .map_err(|issue| vec![issue])?,
            operations: vec![Operation {
                id: "run".into(),
                inputs: legacy.inputs.clone(),
                params: legacy
                    .params
                    .iter()
                    .map(OperationParam::try_from)
                    .collect::<Result<_, _>>()
                    .map_err(|issue| vec![issue])?,
                outputs: legacy.outputs.clone(),
                references: vec![],
                permissions: legacy.permissions.clone(),
                limits: legacy.limits.clone(),
                effects: Effects {
                    mutates_project: false,
                    writes_resources: true,
                    retry_safe: false,
                    undoable: false,
                },
                execution: Execution {
                    background: legacy.tool.web_worker,
                    cancellable: false,
                    resumable: false,
                    max_memory_bytes: None,
                },
            }],
        })
    }

    pub fn operation(&self, id: &str) -> Option<&Operation> {
        self.operations.iter().find(|op| op.id == id)
    }

    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = vec![];
        if self.schema != SCHEMA {
            issues.push(Issue("version de contrat v2 inconnue".into()));
        }
        if self.operations.is_empty() {
            issues.push(Issue("au moins une opération est requise".into()));
        }
        let mut ids = BTreeSet::new();
        for op in &self.operations {
            if op.id.is_empty()
                || op.id.len() > 128
                || !op
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                || !ids.insert(&op.id)
            {
                issues.push(Issue(format!(
                    "opération invalide ou dupliquée : {}",
                    op.id
                )));
            }
            let view = ToolContract {
                schema: 1,
                tool: self.tool.clone(),
                search: Default::default(),
                limits: op.limits.clone(),
                inputs: op.inputs.clone(),
                params: op
                    .params
                    .iter()
                    .map(|param| {
                        Ok(Param {
                            id: param.id.clone(),
                            ty: param.ty.clone(),
                            level: param.level,
                            default: param.default.as_ref().map(json_toml).transpose()?,
                            range: param.range.clone(),
                            options: param.options.clone(),
                            label: param.label.clone(),
                        })
                    })
                    .collect::<Result<_, Issue>>()
                    .unwrap_or_else(|issue| {
                        issues.push(issue);
                        vec![]
                    }),
                outputs: op.outputs.clone(),
                permissions: op.permissions.clone(),
                variants: vec![],
            };
            // Project commands may produce only an updated project reference.
            issues.extend(crate::validate::validate_contract(
                &view,
                std::slice::from_ref(&self.tool.universe),
                false,
            ));
            let mut ports: BTreeSet<&str> = op
                .inputs
                .iter()
                .map(|p| p.id.as_str())
                .chain(op.params.iter().map(|p| p.id.as_str()))
                .chain(op.outputs.iter().map(|p| p.id.as_str()))
                .collect();
            for port in &op.references {
                if port.id.is_empty()
                    || port.id.len() > 128
                    || !port
                        .id
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                    || !ports.insert(&port.id)
                {
                    issues.push(Issue(format!(
                        "référence invalide ou dupliquée : {}",
                        port.id
                    )));
                }
                let valid = match &port.kind {
                    ReferenceKind::Resource { mime } => mime.contains('/'),
                    ReferenceKind::Project { format, schema } => !format.is_empty() && *schema > 0,
                };
                if !valid {
                    issues.push(Issue(format!("type de référence invalide : {}", port.id)));
                }
            }
            if op.execution.max_memory_bytes == Some(0)
                || (op.execution.resumable && !op.execution.background)
                || (op.effects.undoable && !op.effects.mutates_project)
            {
                issues.push(Issue(format!("capacités incohérentes : {}", op.id)));
            }
        }
        issues
    }
}
