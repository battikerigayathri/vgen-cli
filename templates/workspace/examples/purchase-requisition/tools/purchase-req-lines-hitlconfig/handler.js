try {
  const slug = "purchase-req-lines-form";

  const result = await queryRecords([
    { collectionName: "hitlConfig", query: { slug: slug } },
  ]);

  // Dual-unwrap: queryRecords returns a nested 2D array, sometimes wrapped in { data }.
  // See docs/sdk-response-patterns.md §1.
  const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];

  if (!record) {
    return JSON.stringify({
      success: false,
      error: "HITL record not found for slug: " + slug,
      agentResponseContext: "Push hitl/purchase-req-lines-form first.",
    });
  }

  const configObj =
    typeof record.config === "string"
      ? JSON.parse(record.config)
      : record.config || {};

  return JSON.stringify({
    success: true,
    config: JSON.stringify(configObj),
    preMessage: record.preMessage || "",
    postMessage: record.postMessage || "",
    values: {},
    agentResponseContext: "Show the line items collect card.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
