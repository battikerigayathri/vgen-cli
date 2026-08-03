try {
  const input = context?.input || {};
  const vendorId = (input.vendorId || input.vendor_id || "").toString().trim();
  if (!vendorId) {
    return JSON.stringify({
      success: false,
      error: "vendorId is required",
      agentResponseContext: "Collect vendor ID via HITL before saving.",
    });
  }

  return JSON.stringify({
    success: true,
    workflowPatch: {
      inputs: { vendorId: vendorId },
    },
    agentResponseContext:
      "Vendor saved to workflow inputs. Platform advances when collect_vendor doneWhen is satisfied.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
