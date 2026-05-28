#[derive(Debug, Clone)]
pub struct DepInfo {
    pub name: &'static str,
    pub required: bool,
    pub installed: bool,
    pub version: Option<String>,
}

impl DepInfo {
    pub fn status_text(&self) -> &str {
        if self.installed {
            "OK"
        } else if self.required {
            "MISSING"
        } else {
            "optional"
        }
    }
}

pub fn check_dependencies() -> Vec<DepInfo> {
    let mut deps = Vec::new();

    deps.push(check_lib("libavcodec", true));
    deps.push(check_lib("libavformat", true));
    deps.push(check_lib("libswscale", true));

    deps
}

fn check_lib(name: &'static str, required: bool) -> DepInfo {
    let output = std::process::Command::new("pkg-config")
        .args(["--modversion", name])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let version = stdout.lines().next().map(|s| s.trim().to_string());
            DepInfo {
                name,
                required,
                installed: true,
                version,
            }
        }
        _ => DepInfo {
            name,
            required,
            installed: false,
            version: None,
        },
    }
}

pub fn has_missing_required(deps: &[DepInfo]) -> bool {
    deps.iter().any(|d| d.required && !d.installed)
}

pub fn format_dep_report(deps: &[DepInfo]) -> String {
    let mut lines = Vec::new();
    lines.push("Screenshot Daemon - Dependency Check".to_string());
    lines.push(String::new());
    for dep in deps {
        let ver = dep.version.as_deref().unwrap_or("unknown");
        let req = if dep.required { "required" } else { "optional" };
        lines.push(format!(
            "  {:15} [{}] {} ({})",
            dep.name,
            dep.status_text(),
            ver,
            req
        ));
    }
    lines.join("\n")
}
