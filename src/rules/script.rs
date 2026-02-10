use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use rhai::{AST, Dynamic, Engine};

/// Create a sandboxed Rhai engine with resource limits.
pub fn create_script_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_call_levels(16);
    engine.set_max_expr_depths(64, 64);
    engine.set_max_string_size(10_000);
    engine.set_max_array_size(1_000);
    engine
}

/// The script must define `fn detect(value)` returning an array of strings.
/// Validates via dry-run call to catch signature and return-type errors at
/// load time.
pub fn compile_script(engine: &Engine, path: &Path) -> Result<AST> {
    let source = std::fs::read_to_string(path).with_context(|| format!("failed to read script: {}", path.display()))?;
    let ast = engine
        .compile(&source)
        .map_err(|e| anyhow!("failed to compile script '{}': {e}", path.display()))?;

    validate_script(engine, &ast, path)?;

    Ok(ast)
}

fn validate_script(engine: &Engine, ast: &AST, path: &Path) -> Result<()> {
    let detect_fn = ast.iter_functions().find(|f| f.name == "detect");

    let meta = detect_fn.ok_or_else(|| anyhow!("script '{}' must define `fn detect(value)`", path.display()))?;

    if meta.params.len() != 1 {
        bail!(
            "script '{}': detect() must take exactly 1 parameter, found {}",
            path.display(),
            meta.params.len()
        );
    }

    let result: Dynamic = engine
        .call_fn(&mut rhai::Scope::new(), ast, "detect", (String::new(),))
        .map_err(|e| anyhow!("script '{}': dry-run of detect(\"\") failed: {e}", path.display()))?;

    if result.into_typed_array::<String>().is_err() {
        bail!("script '{}': detect() must return an array of strings", path.display());
    }

    Ok(())
}

pub fn run_detect(engine: &Engine, ast: &AST, value: &str) -> Result<Vec<String>> {
    let result: Dynamic = engine
        .call_fn(&mut rhai::Scope::new(), ast, "detect", (value.to_string(),))
        .map_err(|e| anyhow!("failed to call detect(value): {e}"))?;

    if let Ok(arr) = result.into_typed_array::<String>() {
        return Ok(arr);
    }

    bail!("detect() must return an array of strings")
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_temp_script(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn compile_and_run_detect() {
        let engine = create_script_engine();
        let script = write_temp_script(
            r#"
            fn detect(value) {
                let results = [];
                if value.contains("SECRET") {
                    results.push("SECRET");
                }
                results
            }
            "#,
        );
        let ast = compile_script(&engine, script.path()).unwrap();

        let matches = run_detect(&engine, &ast, "has SECRET here").unwrap();
        assert_eq!(matches, vec!["SECRET"]);

        let matches = run_detect(&engine, &ast, "nothing here").unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn compile_invalid_script_fails() {
        let engine = create_script_engine();
        let script = write_temp_script("fn broken( {");
        assert!(compile_script(&engine, script.path()).is_err());
    }

    #[test]
    fn missing_detect_fn_rejected() {
        let engine = create_script_engine();
        let script = write_temp_script("fn other(x) { x }");
        let err = compile_script(&engine, script.path()).unwrap_err();
        assert!(err.to_string().contains("must define"), "{err}");
    }

    #[test]
    fn wrong_param_count_rejected() {
        let engine = create_script_engine();
        let script = write_temp_script("fn detect(a, b) { [] }");
        let err = compile_script(&engine, script.path()).unwrap_err();
        assert!(err.to_string().contains("exactly 1 parameter"), "{err}");
    }

    #[test]
    fn wrong_return_type_rejected() {
        let engine = create_script_engine();
        let script = write_temp_script(r#"fn detect(value) { "not an array" }"#);
        let err = compile_script(&engine, script.path()).unwrap_err();
        assert!(err.to_string().contains("array of strings"), "{err}");
    }

    #[test]
    fn sandbox_limits_operations() {
        let engine = create_script_engine();
        let script = write_temp_script(
            r#"
            fn detect(value) {
                let i = 0;
                loop { i += 1; }
                []
            }
            "#,
        );
        // Infinite loop is caught during dry-run validation at compile time
        let err = compile_script(&engine, script.path()).unwrap_err();
        assert!(err.to_string().contains("Too many operations"), "{err}");
    }
}
