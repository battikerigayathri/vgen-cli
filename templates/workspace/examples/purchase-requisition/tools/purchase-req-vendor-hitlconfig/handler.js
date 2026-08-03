try {
  const input = context?.input || {};
  const vendorId = (input.vendorId || input.vendor_id || "").toString().trim();
  const slug = "purchase-req-vendor-form";

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
      agentResponseContext:
        "Push hitl/purchase-req-vendor-form before using this tool.",
    });
  }

  let configObj =
    typeof record.config === "string"
      ? JSON.parse(record.config)
      : record.config || {};

  let configStr = JSON.stringify(configObj);
  if (vendorId) {
    configStr = configStr.replace(/\$\{vendorId\}/g, vendorId);
    configObj = JSON.parse(configStr);
  }

  return JSON.stringify({
    success: true,
    config: JSON.stringify(configObj),
    preMessage: record.preMessage || "",
    postMessage: record.postMessage || "",
    values: vendorId ? { vendorId: vendorId } : {},
    agentResponseContext: "Show the vendor collect card for purchase requisition.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
