try {
  const input = context?.input || {};
  let lineItems = input.lineItems ?? input.line_items;

  if (typeof lineItems === "string") {
    const trimmed = lineItems.trim();
    if (!trimmed) {
      return JSON.stringify({
        success: false,
        error: "lineItems is required",
        agentResponseContext: "Collect line items via HITL before saving.",
      });
    }
    lineItems = JSON.parse(trimmed);
  }

  if (!Array.isArray(lineItems) || lineItems.length === 0) {
    return JSON.stringify({
      success: false,
      error: "lineItems must be a non-empty array",
      agentResponseContext: "Ask the user for at least one line item.",
    });
  }

  return JSON.stringify({
    success: true,
    workflowPatch: {
      inputs: { lineItems: lineItems },
    },
    agentResponseContext:
      "Line items saved. Platform advances to review when collect_line_items doneWhen is satisfied.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
