import { smriti } from "/runtime/runtime-sdks/smriti.js";
import axios from "axios";

/**
 * Jira Read Issue - FaaS handler (ES module)
 * Expects event.context with context.input.issueKey (or issue_key).
 * Reads per-session secret via smriti.secrets.get using key:
 *   auth-token-${context.assigned_agent}-${context.roc_user_session.id}
 */

async function handler(event) {
    const context = event && event.context ? event.context : {};

    try {
        const input = context.input || {};
        const issueKey = (input.issueKey || input.issue_key || "")
            .toString()
            .trim();
        if (!issueKey) {
            return {
                success: false,
                error: "issueKey is required (e.g. PROJ-123)",
                agentResponseContext:
                    "Ask the user for the Jira issue key to summarize.",
            };
        }

        // Build secret key and fetch credential
        const rocId =
            context.roc_user_session &&
                (context.roc_user_session.id ||
                    context.roc_user_session.sessionId ||
                    context.roc_user_session.userId)
                ? context.roc_user_session.id ||
                context.roc_user_session.sessionId ||
                context.roc_user_session.userId
                : "";
        const secretKey = `auth-token-${context.assigned_agent}-${rocId}`;

        const secretData = await smriti.secrets.get({
            key: secretKey,
            servicetype: "AwsSecretsManager",
        });

        // FaaS secrets return { data: { value } } — see docs/sdk-response-patterns.md §2.
        const credential = secretData?.data?.value;
        if (!credential) {
            return {
                success: false,
                error: "Jira API credential not found for this session.",
                agentResponseContext:
                    "The Jira API credential is missing. Ensure the user has authenticated or the secret exists.",
            };
        }

        const JIRA_BASE_URL =
            process && process.env && process.env.JIRA_BASE_URL
                ? process.env.JIRA_BASE_URL.replace(/\/+$/, "")
                : "https://resmedglobal.atlassian.net";

        const queryFields =
            "summary,assignee,description,reporter,attachment,parent,customfield_10022,priority,status,customfield_11357";
        const url = `${JIRA_BASE_URL}/rest/api/3/issue/${issueKey}?fields=${queryFields}`;

        const headers = {
            Accept: "application/json",
        };

        // Always construct Basic auth from session email and secret value
        const secretVal = String(credential);
        const sessionEmail =
            context.roc_user_session && context.roc_user_session.email
                ? String(context.roc_user_session.email)
                : "";
        if (!sessionEmail) {
            return {
                success: false,
                error:
                    "User email not available in roc_user_session; cannot construct Basic auth.",
                agentResponseContext:
                    "Ensure the user session includes the email used for Jira credentials.",
            };
        }
        const authString = `${sessionEmail}:${secretVal}`;
        const b64 = Buffer.from(authString).toString("base64");
        headers.Authorization = `Basic ${b64}`;

        const res = await axios.get(url, {
            headers,
            timeout: 10000,
            params: { fields: queryFields },
            responseType: "json",
        });

        const issue = res.data || {};
        const issueFields = issue.fields || {};
        const summary = issueFields.summary || "";
        const status = issueFields.status || null;
        const assignee = issueFields.assignee || null;
        const reporter = issueFields.reporter || null;
        const attachment = Array.isArray(issueFields.attachment)
            ? issueFields.attachment
            : [];
        const parent = issueFields.parent || null;
        const priority = issueFields.priority || null;
        const customfield_10022 = issueFields.customfield_10022 || null;
        const customfield_11357 = issueFields.customfield_11357 || null;

        // description may be Atlassian Document Format; stringify fallback
        let description = "";
        if (typeof issueFields.description === "string") {
            description = issueFields.description;
        } else if (issueFields.description && issueFields.description.content) {
            try {
                description = JSON.stringify(issueFields.description);
            } catch (e) {
                description = "";
            }
        }

        const shortDesc = description
            ? description.length > 800
                ? description.slice(0, 800) + "…"
                : description
            : "";

        return {
            success: true,
            issueKey,
            summary,
            status,
            assignee,
            reporter,
            attachment,
            parent,
            customfield_10022,
            priority,
            customfield_11357,
            description: shortDesc,
            message: `Fetched Jira issue ${issueKey}`,
            agentResponseContext: `Provide a short user-friendly summary of Jira issue ${issueKey} using summary, status, assignee, and a brief description.`,
        };
    } catch (err) {
        console.error(
            "Error in jira-read-issue handler:",
            err && err.stack ? err.stack : err,
        );
        return {
            success: false,
            error: err?.message || String(err),
            errorType: err?.name || "Error",
            retryable: false,
        };
    }
}
