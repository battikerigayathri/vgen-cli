try {
  const input = context?.input || {};
  const repoName = (input.repoName || input.repo_name || "").toString().trim();
  const featureDescription = (
    input.featureDescription ||
    input.feature_description ||
    ""
  )
    .toString()
    .trim();

  if (!repoName || !featureDescription) {
    return JSON.stringify({
      success: false,
      error: "repoName and featureDescription are required",
      agentResponseContext: "Collect intent via HITL before saving.",
    });
  }

  return JSON.stringify({
    success: true,
    workflowPatch: {
      inputs: { repoName: repoName, featureDescription: featureDescription },
    },
    agentResponseContext:
      "Intent saved. Platform advances to agent_task when collect_intent doneWhen is satisfied.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
