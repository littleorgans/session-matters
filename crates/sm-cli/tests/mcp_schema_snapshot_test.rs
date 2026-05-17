#[test]
fn mcp_each_tool_snapshot() {
    let tool_list = sm_cli::mcp::schema::tool_list();
    let tools = tool_list["tools"].as_array().expect("tools are an array");
    for tool in tools {
        let name = tool["name"].as_str().expect("tool has a name");
        insta::with_settings!({ snapshot_suffix => name }, {
            insta::assert_json_snapshot!("mcp_tool", tool);
        });
    }
}

#[test]
fn mcp_server_instructions_snapshot() {
    insta::assert_snapshot!(
        "mcp_server_instructions",
        sm_cli::mcp::instructions::SERVER_INSTRUCTIONS
    );
}
