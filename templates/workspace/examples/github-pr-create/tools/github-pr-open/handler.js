try {
  const input = context?.input || {};
  const repoName = (input.repoName || input.repo_name || "org/repo")
    .toString()
    .trim();

  // Mock PR URL for smoke/demo — replace with live GitHub API in production workspaces.
  const prNumber = Math.floor(Math.random() * 900) + 100;
  const prUrl = "https://github.com/" + repoName + "/pull/" + prNumber;

  return JSON.stringify({
    success: true,
    workflowPatch: {
      artifacts: { prUrl: prUrl },
    },
    agentResponseContext:
      "PR opened (mock). Stage stays on agent_task until macro validator requirement_met and platform confirm_validator_pass_and_advance.",
    prUrl: prUrl,
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
