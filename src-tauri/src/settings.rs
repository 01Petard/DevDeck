use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

type StoreValues = HashMap<String, Value>;
type StoreError = Box<dyn std::error::Error + Send + Sync>;

fn valid(values: &StoreValues) -> bool {
    if values.contains_key("__loadError") {
        return false;
    }
    let Some(settings) = values.get("settings") else {
        return values.is_empty();
    };
    if settings.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return false;
    }
    let Some(favorites) = settings.get("favorites").and_then(Value::as_array) else {
        return false;
    };
    let mut ports = HashSet::new();
    favorites.iter().all(|favorite| {
        let Some(port) = favorite
            .get("port")
            .and_then(Value::as_u64)
            .filter(|p| (1..=65535).contains(p))
        else {
            return false;
        };
        let Some(label) = favorite.get("label").and_then(Value::as_str) else {
            return false;
        };
        if !ports.insert(port) || label != label.trim() || label.chars().count() > 60 {
            return false;
        }
        match favorite.get("note") {
            None => {}
            Some(Value::String(note)) if note == note.trim() && note.chars().count() <= 500 => {}
            _ => return false,
        }
        match favorite.get("healthUrl") {
            Some(Value::Null) => true,
            Some(Value::String(raw)) => crate::health::validate_url(raw)
                .is_ok_and(|url| url.port_or_known_default() == Some(port as u16)),
            _ => false,
        }
    })
}

pub fn read_initial_store(path: &Path) -> StoreValues {
    match std::fs::read(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            HashMap::from([("settings".into(), json!({"schemaVersion":1,"favorites":[]}))])
        }
        Ok(bytes) => match serde_json::from_slice::<StoreValues>(&bytes) {
            Ok(values) if valid(&values) => values,
            _ => HashMap::from([("__loadError".into(), json!(true))]),
        },
        Err(_) => HashMap::from([("__loadError".into(), json!(true))]),
    }
}

// Store also saves on app exit. Reject damaged/unsupported data before it can open the file for writing.
pub fn serialize_settings(values: &StoreValues) -> Result<Vec<u8>, StoreError> {
    if !valid(values) {
        return Err(std::io::Error::other("收藏配置不可写，请先修复配置文件。").into());
    }
    Ok(serde_json::to_vec_pretty(values)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_notes_accept_multiline_unicode_and_reject_invalid_persisted_values() {
        let favorite =
            json!({"port":8080,"label":"API","healthUrl":null,"note":"本地联调\n第二行说明"});
        let values = HashMap::from([(
            "settings".into(),
            json!({"schemaVersion":1,"favorites":[favorite.clone()]}),
        )]);
        assert!(serialize_settings(&values).is_ok());
        for note in [
            json!(null),
            json!(500),
            json!([]),
            json!("😀".repeat(501)),
            json!(" leading space"),
        ] {
            let mut invalid = favorite.clone();
            invalid["note"] = note;
            assert!(
                serialize_settings(&HashMap::from([(
                    "settings".into(),
                    json!({"schemaVersion":1,"favorites":[invalid]})
                )]))
                .is_err()
            );
        }
    }

    #[test]
    fn invalid_settings_cannot_be_saved_even_on_exit() {
        let good = json!({"schemaVersion":1,"favorites":[{"port":8080,"label":"API","healthUrl":"http://localhost:8080/health"}]});
        let values = HashMap::from([("settings".into(), good.clone())]);
        assert!(serialize_settings(&values).is_ok());
        for settings in [
            json!(null),
            json!({"schemaVersion":2,"favorites":[]}),
            json!({"schemaVersion":1,"favorites":[{"port":0,"label":"","healthUrl":null}]}),
            json!({"schemaVersion":1,"favorites":[{"port":80,"label":"","healthUrl":"http://example.com"}]}),
            json!({"schemaVersion":1,"favorites":[{"port":81,"label":"","healthUrl":"http://localhost"}]}),
            json!({"schemaVersion":1,"favorites":[{"port":80,"label":"","healthUrl":null},{"port":80,"label":"","healthUrl":null}]}),
            json!({"schemaVersion":1,"favorites":[{"port":80,"label":" a ","healthUrl":null}]}),
            json!({"schemaVersion":1,"favorites":[{"port":80,"label":"a"}]}),
        ] {
            assert!(serialize_settings(&HashMap::from([("settings".into(), settings)])).is_err());
        }
        assert!(serialize_settings(&HashMap::from([("__loadError".into(), json!(true))])).is_err());
    }

    #[test]
    fn corrupt_or_unreadable_files_are_preserved() {
        let path = std::env::temp_dir().join(format!(
            "devdock-settings-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert_eq!(
            read_initial_store(&path).get("settings"),
            Some(&json!({"schemaVersion":1,"favorites":[]}))
        );
        let bytes = b"{broken original config";
        std::fs::write(&path, bytes).unwrap();
        assert!(serialize_settings(&read_initial_store(&path)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(serialize_settings(&read_initial_store(&path)).is_err());
        std::fs::remove_dir(&path).unwrap();
    }
}
