use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAuth {
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
    pub tokens: Option<CodexTokens>,
    pub last_refresh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    pub path: PathBuf,
    pub auth: CodexAuth,
}

pub fn auth_candidate_paths(codex_home: Option<&Path>, home: &Path) -> Vec<PathBuf> {
    if let Some(codex_home) = codex_home {
        return vec![codex_home.join("auth.json")];
    }

    vec![
        home.join(".config").join("codex").join("auth.json"),
        home.join(".codex").join("auth.json"),
    ]
}

pub fn find_auth_state() -> Result<AuthState, String> {
    let home = dirs::home_dir().ok_or_else(|| "找不到用户主目录。".to_string())?;
    let codex_home = std::env::var_os("CODEX_HOME").map(PathBuf::from);
    let paths = auth_candidate_paths(codex_home.as_deref(), &home);

    for path in paths {
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("读取 Codex auth.json 失败：{error}"))?;
        let auth: CodexAuth = serde_json::from_str(&text)
            .map_err(|error| format!("Codex auth.json 不是有效 JSON：{error}"))?;
        if auth.tokens.is_some() {
            return Ok(AuthState { path, auth });
        }
        if auth
            .openai_api_key
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            return Err("Codex 用量需要 ChatGPT 登录，不能使用 API key 登录。".to_string());
        }
    }

    Err("未登录。请使用文件凭据模式运行 `codex login`。".to_string())
}

pub fn save_auth_state(state: &AuthState) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&state.auth)
        .map_err(|error| format!("序列化 Codex auth.json 失败：{error}"))?;
    std::fs::write(&state.path, text).map_err(|error| format!("保存 Codex auth.json 失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn auth_candidates_prefer_codex_home_when_set() {
        let home = PathBuf::from("/home/alex");
        let codex_home = PathBuf::from("/tmp/codex-home");

        let paths = auth_candidate_paths(Some(&codex_home), &home);

        assert_eq!(paths, vec![PathBuf::from("/tmp/codex-home/auth.json")]);
    }

    #[test]
    fn auth_candidates_fall_back_to_config_then_legacy_home() {
        let home = PathBuf::from("/home/alex");

        let paths = auth_candidate_paths(None, &home);

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/alex/.config/codex/auth.json"),
                PathBuf::from("/home/alex/.codex/auth.json"),
            ]
        );
    }
}
