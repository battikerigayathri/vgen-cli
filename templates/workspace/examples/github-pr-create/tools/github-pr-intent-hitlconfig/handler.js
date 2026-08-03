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
  const slug = "github-pr-intent-form";

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
      agentResponseContext: "Push hitl/github-pr-intent-form first.",
    });
  }

  let configObj =
    typeof record.config === "string"
      ? JSON.parse(record.config)
      : record.config || {};

  let configStr = JSON.stringify(configObj);
  if (repoName) {
    configStr = configStr.replace(/\$\{repoName\}/g, repoName);
  }
  if (featureDescription) {
    configStr = configStr.replace(/\$\{featureDescription\}/g, featureDescription);
  }
  configObj = JSON.parse(configStr);

  const values = {};
  if (repoName) values.repoName = repoName;
  if (featureDescription) values.featureDescription = featureDescription;

  return JSON.stringify({
    success: true,
    config: JSON.stringify(configObj),
    preMessage: record.preMessage || "",
    postMessage: record.postMessage || "",
    values: values,
    agentResponseContext: "Show the GitHub PR intent collect card.",
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err),
  });
}
