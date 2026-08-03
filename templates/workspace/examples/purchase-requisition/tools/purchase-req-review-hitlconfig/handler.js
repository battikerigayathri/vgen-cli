try {
  const input = context?.input || {};
  const vendorId = (input.vendorId || input.vendor_id || "").toString().trim();
  let lineItems = input.lineItems ?? input.line_items ?? [];
  if (typeof lineItems === "string") {
    try {
      lineItems = JSON.parse(lineItems);
    } catch (_e) {
      lineItems = [];
    }
  }
  const lineItemsSummary = Array.isArray(lineItems)
    ? JSON.stringify(lineItems, null, 2)
    : String(lineItems || "");

  const slug = "purchase-req-review";
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
      agentResponseContext: "Push hitl/purchase-req-review first.",
    });
  }

  let configObj =
    typeof record.config === "string"
      ? JSON.parse(record.config)
      : record.config || {};

  let configStr = JSON.stringify(configObj);
  configStr = configStr.replace(/\$\{vendorId\}/g, vendorId || "(not set)");
  configStr = configStr.replace(/\$\{lineItemsSummary\}/g, lineItemsSummary || "(none)");
  configObj = JSON.parse(configStr);

  return JSON.stringify({
    success: true,
    config: JSON.stringify(configObj),
    preMessage: record.preMessage || "",
    postMessage: record.postMessage || "",
    values: { vendorId: vendorId, lineItemsSummary: lineItemsSummary },
    agentResponseContext: "Show the review summary card for user confirmation.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
