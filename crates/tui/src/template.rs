use std::collections::HashMap;

pub fn render(text: &str, variables: &HashMap<String, String>) -> String {
    let mut result = text.to_string();

    for (k, v) in variables {
        let p1 = format!("{{{{{}}}}}", k);
        result = result.replace(&p1, v);
    }

    let mut current_idx = 0;
    while let Some(start) = result[current_idx..].find("{{env:") {
        let abs_start = current_idx + start;
        if let Some(end) = result[abs_start..].find("}}") {
            let abs_end = abs_start + end + 2;
            let env_name = &result[abs_start + 6..abs_start + end];
            let env_val = std::env::var(env_name).unwrap_or_else(|_| "".to_string());
            result.replace_range(abs_start..abs_end, &env_val);
            current_idx = abs_start + env_val.len();
        } else {
            break;
        }
    }

    result
}
