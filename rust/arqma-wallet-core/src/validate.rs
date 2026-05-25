use serde_json::Value;

fn is_object (v: &Value) -> bool {
  v.as_object().is_some()
}

/// Same as `Backend.validate_values` — walks `values` (merged) and checks against `defaults`.
pub fn validate_values (values: &Value, defaults: &Value) -> Value {
  let (Some(m), Some(d)) = (values.as_object(), defaults.as_object()) else {
    return values.clone();
  };
  let mut out = m.clone();
  for key in m.keys() {
    if !d.contains_key(key) {
      continue;
    }
    let def_v = d.get(key).unwrap();
    if def_v.is_null() {
      continue;
    }
    let val = m.get(key).unwrap();
    if is_object(val) && is_object(def_v) {
      out.insert(key.clone(), validate_values(val, def_v));
    } else {
      let invalid = val.is_null()
        || (val.as_str().map(|s| s.is_empty()).unwrap_or(false))
        || (val.as_f64().map(|f| f.is_nan()).unwrap_or(false));
      if invalid {
        out.insert(key.clone(), def_v.clone());
      }
    }
  }
  Value::Object(out)
}

/// `daemons` / `app` / `wallet` — separate `validate_values` passes against `defaults`.
pub fn validate_config_against_defaults (config_data: &Value, defaults: &Value) -> Value {
  if !is_object(config_data) || !is_object(defaults) {
    return config_data.clone();
  }
  let c = config_data.as_object().unwrap();
  let d = defaults.as_object().unwrap();
  let mut out = c.clone();
  for (k, def_v) in d {
    if !out.contains_key(k) {
      continue;
    }
    if let Some(a) = c.get(k) {
      if is_object(a) && is_object(def_v) {
        out.insert(k.clone(), validate_values(a, def_v));
      }
    }
  }
  Value::Object(out)
}
