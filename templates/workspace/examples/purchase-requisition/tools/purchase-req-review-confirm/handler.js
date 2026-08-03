try {
  const input = context?.input || {};
  const raw = input.reviewConfirmed ?? input.review_confirmed;
  const confirmed =
    raw === true ||
    raw === "true" ||
    raw === "True" ||
    raw === 1 ||
    raw === "1";

  if (!confirmed) {
    return JSON.stringify({
      success: false,
      error: "reviewConfirmed must be true",
      agentResponseContext:
        "User must confirm the review summary before submit.",
    });
  }

  return JSON.stringify({
    success: true,
    workflowPatch: {
      inputs: { reviewConfirmed: true },
    },
    agentResponseContext:
      "Review confirmed. Platform advances to submit when completeness gate passes.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
