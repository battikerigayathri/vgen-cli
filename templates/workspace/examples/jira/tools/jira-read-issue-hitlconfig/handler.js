try {
  const input = context?.input || {};
  const issueKey = (input.issueKey || input.issue_key || "").toString().trim();
  const slug = "jira-read-issue-confirm";

  const result = await queryRecords([
    {
      collectionName: "hitlConfig",
      query: { slug: slug },
    },
  ]);

  // Dual-unwrap: queryRecords returns a nested 2D array, sometimes wrapped in { data }.
  // See docs/sdk-response-patterns.md §1.
  const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];

  if (!record) {
    return JSON.stringify({
      success: false,
      error: "HITL record not found for slug: " + slug,
      agentResponseContext:
        "The read-issue confirmation form is not configured. Push the hitl/jira-read-issue-confirm record first.",
    });
  }

  let configObj =
    typeof record.config === "string"
      ? JSON.parse(record.config)
      : record.config || {};

  let configStr = JSON.stringify(configObj);
  if (issueKey) {
    configStr = configStr.replace(/\$\{issueKey\}/g, issueKey);
    configObj = JSON.parse(configStr);
  }

  const values = issueKey ? { issueKey: issueKey } : {};

  return JSON.stringify({
    success: true,
    config: JSON.stringify(configObj),
    preMessage: record.preMessage || "",
    postMessage: record.postMessage || "",
    values: values,
    agentResponseContext: issueKey
      ? "Show the read-issue confirmation card with issue key pre-filled."
      : "Show the read-issue confirmation card so the user can enter an issue key.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
