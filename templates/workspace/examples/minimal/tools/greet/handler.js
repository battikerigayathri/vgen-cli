try {
  const input = context?.input || {};
  const name = (input.name || "").toString().trim();
  if (!name) {
    return JSON.stringify({
      success: false,
      error: "name is required",
      agentResponseContext: "Ask the user for their name.",
    });
  }
  return JSON.stringify({
    success: true,
    greeting: "Hello, " + name + "!",
    agentResponseContext: "Reply with the greeting warmly and briefly.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
