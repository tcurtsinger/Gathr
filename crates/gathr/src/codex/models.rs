//! Account catalog and Gathr's family Latest aliases (0033, owner task 1291).
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const LATEST: &str = "latest:";

/// Only gpt-<numeric version>-<lowercase family> is grouped. Unknown shapes stay exact.
fn family(id: &str) -> Option<(&str, Vec<u64>)> {
    let (version, family) = id.strip_prefix("gpt-")?.rsplit_once('-')?;
    if family.is_empty() || !family.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let mut version = version
        .split('.')
        .map(|p| {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        })
        .collect::<Option<Vec<u64>>>()?;
    while version.len() > 1 && version.last() == Some(&0) {
        version.pop();
    }
    Some((family, version))
}

/// Every native row stays, plus one moving alias for each family in this account's list.
pub fn with_latest(exact: Vec<Value>) -> Vec<Value> {
    let mut newest = BTreeMap::new();
    for model in &exact {
        let Some(id) = model["id"].as_str() else {
            continue;
        };
        let Some((family, version)) = family(id) else {
            continue;
        };
        let key = (version, id);
        let replace = newest.get(family).is_none_or(|(was, _)| &key > was);
        if replace {
            newest.insert(family, (key, model));
        }
    }
    let mut models = Vec::new();
    for (family, (_, model)) in newest {
        let mut row = model.clone();
        row["id"] = format!("{LATEST}{family}").into();
        row["latest"] = true.into();
        let name = format!(
            "{}{} Latest",
            family[..1].to_ascii_uppercase(),
            &family[1..]
        );
        row["name"] = name.into();
        models.push(row);
    }
    models.extend(exact);
    models
}

/// The same mapping feeds Settings and resolves aliases before a chat thread opens.
pub fn listed(raw: &[Value]) -> Result<Vec<Value>, String> {
    let row = |m: &Value| {
        json!({
            "id":m["model"], "resolved":m["model"], "latest":false,
            "name":m["displayName"], "description":m["description"],
            "efforts":m["supportedReasoningEfforts"].as_array().into_iter().flatten()
                .filter_map(|e| e["reasoningEffort"].as_str()).collect::<Vec<_>>(),
        })
    };
    let all: Vec<_> = raw.iter().filter(|m| m["hidden"] != true).collect();
    let default = all
        .iter()
        .find(|m| m["isDefault"] == true)
        .ok_or("Codex returned no default model")?;
    let mut first = row(default);
    first["id"] = "default".into();
    first["name"] = "Engine default".into();
    let mut models = vec![first];
    models.extend(with_latest(all.into_iter().map(row).collect()));
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_rows_follow_the_real_signed_in_catalog_and_keep_the_exact_rows() {
        let actual: Value = serde_json::from_str(include_str!(
            "../../tests/frames/codex_signed_in_models.json"
        ))
        .unwrap();
        let exact = actual["models"].as_array().unwrap().clone();
        let listed = with_latest(exact.clone());
        let aliases: Vec<_> = listed
            .iter()
            .filter(|m| m["latest"] == true)
            .map(|m| (m["name"].as_str().unwrap(), m["resolved"].as_str().unwrap()))
            .collect();
        assert_eq!(
            aliases,
            [
                ("Astra Latest", "gpt-6-astra"),
                ("Luna Latest", "gpt-6-luna"),
                ("Sol Latest", "gpt-6.1-sol"),
                ("Terra Latest", "gpt-6.1-terra")
            ]
        );
        assert_eq!(&listed[4..], exact);
    }
    #[test]
    fn numeric_versions_move_forward_and_unfamiliar_names_stay_exact() {
        let ids = [
            "gpt-6.9-sol",
            "gpt-6.10-sol",
            "gpt-6.10.1-sol",
            "gpt-7-sol-preview",
            "gpt-8.0-SOL",
            "gpt-future-sol",
            "o9",
            "gpt-8.0-sol2",
        ];
        let rows = ids.map(|id| json!({"id":id,"resolved":id,"latest":false}));
        let listed = with_latest(rows.to_vec());
        assert_eq!(listed.len(), 9, "unexpected grouping: {listed:?}");
        assert_eq!(listed[0]["resolved"], "gpt-6.10.1-sol");
        assert_eq!(&listed[1..], rows);
    }
}
