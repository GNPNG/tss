use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

const CATALOG: &str = include_str!("../../WebAssets/assets/visual-advice.json");
const MODEL: &str = "local-advisor:functiongemma";

#[derive(Serialize)]
pub struct Suggestion {
    visual: String,
    signal: String,
    fit: String,
}

fn routing_request(direction: &str, axis: &str, choices: &serde_json::Map<String, Value>) -> Value {
    let tools: Vec<_> = choices.iter().map(|(id, description)| json!({
        "type":"function", "function": { "name":format!("choose_{id}"), "description":description,
        "parameters":{"type":"object","properties":{}} }
    })).collect();
    json!({"model":MODEL,"temperature":0,"seed":42,"max_tokens":40,"stream":false,
    "stop":["<end_function_call>"],"tools":tools,"messages":[
        {"role":"developer","content":"You are a model that can do function calling with the following functions"},
        {"role":"user","content":format!("{direction} Choose the {axis} for this direction.")}
    ]})
}

fn read_choice(body: &Value, choices: &serde_json::Map<String, Value>) -> Result<String, String> {
    let completion = body["choices"]
        .as_array()
        .filter(|v| v.len() == 1)
        .and_then(|a| a.first())
        .ok_or("The advisor did not return one choice.")?;
    if completion["finish_reason"] != "stop" {
        return Err("The advisor returned an incomplete choice.".into());
    }
    // llama.cpp excludes the configured stop sequence; either spelling must contain exactly one call.
    let content = completion["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim();
    let call = content
        .strip_suffix("<end_function_call>")
        .unwrap_or(content);
    let id = call
        .strip_prefix("<start_function_call>call:choose_")
        .and_then(|s| s.strip_suffix("{}"))
        .filter(|id| choices.contains_key(*id))
        .ok_or("The advisor returned an unsupported or ambiguous choice.")?;
    Ok(id.to_owned())
}

#[tauri::command]
pub async fn suggest_visual_layer(direction: String, port: u16) -> Result<Suggestion, String> {
    let direction = direction.trim();
    if direction.is_empty() || direction.chars().count() > 1600 || port == 0 {
        return Err("Enter a visual direction of up to 1600 characters and a local port.".into());
    }
    // Local-advisor's FunctionGemma exercise is an optional process started by the operator.
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(40))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|e| e.to_string())?;
    async fn classify(
        client: &reqwest::Client,
        url: &str,
        catalog: &Value,
        axis: &str,
        direction: &str,
    ) -> Result<String, String> {
        let choices = catalog[axis]
            .as_object()
            .ok_or("Invalid bundled advice catalog")?;
        let mut response = client
            .post(url)
            .json(&routing_request(direction, axis, choices))
            .send()
            .await
            .map_err(|e| format!("Could not reach the local FunctionGemma advisor: {e}"))?
            .error_for_status()
            .map_err(|e| format!("The local advisor rejected the request: {e}"))?;
        let mut data = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if data.len() + chunk.len() > 65536 {
                return Err("The advisor response exceeded 64 KiB.".into());
            }
            data.extend_from_slice(&chunk);
        }
        let body =
            serde_json::from_slice(&data).map_err(|e| format!("Invalid advisor response: {e}"))?;
        read_choice(&body, choices).map_err(|e| format!("{axis}: {e}"))
    }
    // Consume one inference slot at a time, matching the service profile.
    Ok(Suggestion {
        visual: classify(&client, &url, &catalog, "visual", direction).await?,
        signal: classify(&client, &url, &catalog, "signal", direction).await?,
        fit: classify(&client, &url, &catalog, "fit", direction).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the local-advisor FunctionGemma process and a fresh evidence pathname"]
    fn evaluate_local_runtime() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../experiments/visual-advisor/holdout.json"
        ))
        .unwrap();
        let mut results = Vec::new();
        for expected in cases {
            let result = tauri::async_runtime::block_on(suggest_visual_layer(
                expected["direction"].as_str().unwrap().into(),
                18081,
            ))
            .unwrap();
            let actual = serde_json::to_value(result).unwrap();
            let correct: Vec<_> = ["visual", "signal", "fit"]
                .into_iter()
                .filter(|axis| actual[*axis] == expected[*axis])
                .collect();
            results.push(json!({"expected":expected,"received":actual,"correct":correct}));
        }
        let path = std::env::var_os("TSS_ADVICE_REPORT").expect("choose fresh evidence file");
        let file = std::fs::File::create_new(path).unwrap();
        serde_json::to_writer_pretty(file, &results).unwrap();
    }
    fn response(s: &str) -> Value {
        json!({"choices":[{"finish_reason":"stop","message":{"content":s}}]})
    }
    #[test]
    fn ambiguous_or_unknown_calls_cannot_be_applied() {
        let catalog: Value = serde_json::from_str(CATALOG).unwrap();
        let visual = catalog["visual"].as_object().unwrap();
        let call = "<start_function_call>call:choose_oil{}";
        assert_eq!(read_choice(&response(call), visual).unwrap(), "oil");
        assert!(read_choice(
            &response(&format!("{call}<end_function_call>{call}")),
            visual
        )
        .is_err());
        assert!(read_choice(
            &response("<start_function_call>call:choose_layer{}"),
            visual
        )
        .is_err());
        assert!(read_choice(
            &json!({"choices":[{"finish_reason":"length","message":{"content":call}}]}),
            visual
        )
        .is_err());
    }
}
