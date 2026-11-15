sprint2_fixture_artifacts_missing() {
  local repo_root="$1"
  [ ! -x "$repo_root/target/debug/fusionpanel-api" ] \
    || [ ! -x "$repo_root/target/debug/fusionpanel-cli" ] \
    || [ ! -x "$repo_root/target/debug/fusionpanel-agent" ] \
    || [ ! -f "$repo_root/frontend/dist/index.html" ]
}
