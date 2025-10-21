use crate::command::init::options::ProjectTemplateOpt;

/// Characterization tests for current MCP behavior
/// These tests document existing behavior before refactoring

#[test]
fn test_mcp_flag_default_false() {
    let template_opt = ProjectTemplateOpt::default();
    assert!(!template_opt.mcp);
}

#[test]
fn test_mcp_flag_behavior() {
    let mcp_enabled = ProjectTemplateOpt {
        template: None,
        mcp: true,
    };
    assert!(mcp_enabled.mcp);

    let mcp_disabled = ProjectTemplateOpt {
        template: None,
        mcp: false,
    };
    assert!(!mcp_disabled.mcp);
}
