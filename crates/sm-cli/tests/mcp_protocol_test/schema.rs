#[test]
pub(crate) fn generated_schema_matches_contract_registry() {
    assert_eq!(
        sm_cli::mcp::schema::tool_list(),
        sm_cli::tool_contracts::contract_registry().tool_list_value()
    );
}
