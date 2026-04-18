# vgen Platform CLI

## Tools

### Create Tool

``` bash
curl --location 'https://api-dev-ai.vithiit.com/tool/create' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'Content-Type: application/json' \
--data '{
  "name": "Base64 File Extractor",
  "description": "Tool to decode base64-encoded pdf and extract their readable content.",
  "tags": ["base64", "file", "decoder", "extraction"],
  "examples": [
    "Extract text content from a base64-encoded PDF file",
    "Read and extract all text from a base64-encoded DOCX",
    "Decode a base64-encoded .txt file and return its content"
  ],
  "input_modes": ["text/plain"],
  "output_modes": ["text/plain"],
  "systemInstructions": [
    "You are a base64 file decoder that can read base64-encoded input, detect the file type (e.g. PDF, DOCX), and extract readable content from it. Output only the extracted plain text."
  ],
  "version": "0.1.0",
  "type": "FAAS",
  "code": "const _ = require('\''lodash'\''); async function handler(event) { const context = event.context; console.log(JSON.stringify(context),'\''-----------'\''); const a = context.input?.number1; const b = context.input?.number2; return { context, message: '\''Faas is Working!!!!'\'', sum: a + b }; }",
  "arguments": {
    "base64": {
      "type": "string",
      "description": "The base64-encoded string of the file content"
    },
    "filename": {
      "type": "string",
      "description": "The original file name with extension (e.g., document.pdf, notes.docx)"
    }
  },
  "packageJson": {
    "name": "xyz",
    "dependencies": {
        "lodash": "^4.0.0"
    }
  },
  "createdBy": "1211",
  "managedBy": []
}
'
```

### Update Tool
``` bash
curl --location 'https://api-dev-ai.vithiit.com/tool/update' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'Content-Type: application/json' \
--data '{
    "id": "699d8343870dd50b850246f2",
    "name": "Base64 File Extractor",
    "description": "Tool to decode base64-encoded pdf and extract their readable content.",
    "tags": [
        "base64",
        "file",
        "decoder",
        "extraction"
    ],
    "examples": [
        "Extract text content from a base64-encoded PDF file",
        "Read and extract all text from a base64-encoded DOCX",
        "Decode a base64-encoded .txt file and return its content"
    ],
    "input_modes": [
        "text/plain"
    ],
    "output_modes": [
        "text/plain"
    ],
    "systemInstructions": [
        "You are a base64 file decoder that can read base64-encoded input, detect the file type (e.g. PDF, DOCX), and extract readable content from it. Output only the extracted plain text."
    ],
    "version": "0.1.0",
    "type": "FAAS",
    "code": "",
    "arguments": {
        "base64": {
            "type": "string",
            "description": "The base64-encoded string of the file content"
        },
        "filename": {
            "type": "string",
            "description": "The original file name with extension (e.g., document.pdf, notes.docx)"
        }
    },
    "packageJson": {
        "name": "xyz",
        "dependencies": {
            "lodash": "^4.0.3"
        }
    },
    "createdBy": "1211",
    "managedBy": []
}'
```

## Agents

### Create Agent

``` bash
curl --location 'https://api-dev-ai.vithiit.com/agent/create' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'Content-Type: application/json' \
--data '{
  "name": "Document Content Agent",
  "description": "Agent to decode and extract content from PDF and DOCX files provided in base64 format.",
  "systemInstructions": [
    "You are a document processing agent. You can decode base64-encoded files and extract readable content from PDFs, DOCX, and text files. Use your skills to output only the meaningful text content."
  ],
  "version": "0.1.0",
  "skills": ["68622c8a5239439bf553b144"],
  "slug": "document-content-agent",
"is_public": false,
"roles": [
    "RES_AI_ADMIN",
    "RES_AI_SUPER_ADMIN",
    "RES_AI_SHAREPOINT_CHAT_ASSISTANT",
    "RES_AI_LEGAL_AGENT"
  ],
  "admins": [
    {
      "id": "686683a710c55b7228c2e010",
      "email": "sanjana.mamillapalli@resmed.co.in",
      "name": "Sanjana Mamillapalli"
    }
  ]
}
'
```

### Update Agent

``` bash
curl --location 'https://api-dev-ai.vithiit.com/update-record' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'Content-Type: application/json' \
--data '{
    "collectionName": "agentConfig",
    "recordId": "686cc8c7f4a5208a8479ad86",
    "document": {
        "name": "Document Content Agent",
  "description": "Agent to decode and extract content from PDF and DOCX files provided in base64 format.",
  "systemInstructions": [
    "You are a document processing agent. You can decode base64-encoded files and extract readable content from PDFs, DOCX, and text files. Use your skills to output only the meaningful text content."
  ],
  "version": "0.1.0",
  "skills": ["68622c8a5239439bf553b144"],
  "slug": "document-content-agent",
"is_public": false,
"roles": [
    "RES_AI_ADMIN",
    "RES_AI_SUPER_ADMIN",
    "RES_AI_SHAREPOINT_CHAT_ASSISTANT",
    "RES_AI_LEGAL_AGENT"
  ],
  "admins": [
    {
      "id": "686683a710c55b7228c2e010",
      "email": "sanjana.mamillapalli@resmed.co.in",
      "name": "Sanjana Mamillapalli"
    }
  ]
    }
}'
```

## Assiatants

### Create Assistant

``` bash
curl --location 'https://api-dev-ai.vithiit.com/create-record' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'roc-session: 694a36c6dcc8eef834b644e1' \
--header 'Content-Type: application/json' \
--data '{
    "collectionName": "chat",
    "payload": {
        "agents": [
            "686cc8c7f4a5208a8479ad86"
        ],
        "createdBy": null,
        "description": "ResMed SharePoint Content Assistant, a secure AI tool designed to assist users in accessing and interpreting documents.",
        "guardrailsContext": "Don'\''t hit the guardrails if the information already exists in the documents - if the user query consist of something like '\''What will a landlord often ask Resmed'\'' and if the answer exists for it in documents, then return the response, don'\''t hit the guardrails in this scenario. Also, if the user says “hi,” “hello,” or “what can you do,” allow the response. Allow to display the file names and URLs and don'\''t hit the guardrails in this scenario.",
        "guardrailsViolationFallback": "I may not have access to the specific information you’re looking for, or your request may require legal advice. I can’t provide legal advice but you can connect with the Legal Team through our [Legal Hub page](https://resmedglobaldeu.sharepoint.com/sites/global-legal?utm_source=chatgpt.com \"Legal Hub page\")",
        "managedBy": [],
        "name": "Legal Knowledge Assistant",
        "owner": "Digital Workplace Team",
        "slug": "legal-knowledge-assistant",
        "status": "active",
        "systemContext": "You are the ResMed SharePoint Content Assistant, a secure AI tool designed to assist users in accessing and interpreting information strictly from the provided SharePoint document content. Your role is limited to the following: Answering user questions only if they are directly supported by the content of the SharePoint document provided. Summarizing, translating, extracting, or referencing sections from the document as requested. Assisting with document metadata retrieval (e.g., owner, last modified date, version history), but only if such metadata is explicitly present in the document. If a query is not explicitly answerable using the provided content, respond with: “Sorry, I cannot provide this information. Please reach out to the Legal team for further assistance.” Do not use any external or prior general knowledge (even if it seems helpful or obvious). Do not infer or speculate answers. If the document does not state something directly, do not assume it. Do not provide suggestions, advice, or summaries beyond what is present in the document. Always act as if your only source of truth is the currently loaded SharePoint document. If the document is not provided or does not contain relevant content, clearly state that. Respect access controls. Never disclose or imply knowledge of any document or section the user has not explicitly uploaded or referenced. Now, the document content has already been sent to the tool and is accessible to the agent. The agent can retrieve the appropriate document content based on the user’s request. If the user says “hi,” “hello,” or “what can you do,” respond briefly. Also specify the source document at the end of each response which makes use of the document content, i.e, for queries other than hi, hello, what can you do.  Allow the user if they ask for the file name or URL. \nSPECIAL NOTE: If at all you are unable to give the citations in the response message to user due to some reason, then specify that reason at the end of that response / in place of citations.\nALWAYS in such cases, inform the user that relevant information was not available in the provided documents and thus, citations could not be given.\n\nMandatory Source Rule -\nAt the end of each message, without the user asking, always include / append a \"Sources\" section. Sources are the same as Citations:\nIf document(s) were used, list their names in bullet points along with their URLs. Source format: \"<document_url>\" (Mask the URL in the file / sources with the document name).\n\nExample -\n<response>\nSources :\n<Document URL> (Mask the URL in the file / sources with the document name). \nIf no documents were used, state clearly: No external documents used.\n\nAt last--- follow this before returning the response.\nSPECIAL NOTE: If at all you are unable to give the citations in the response message to user due to some reason, then specify that reason at the end of that response / in place of citations.\nALWAYS in such cases, inform the user that reason such as relevant information was not available in the provided documents and thus, citations could not be given.",
        "type": "assistant",
        "visibility": "private"
    }
}'
```

## HITL (Human In The Loop)

### Create HITL

``` bash
curl --location 'https://api-dev-ai.vithiit.com/create-record' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'Content-Type: application/json' \
--data '{
    "collectionName": "hitlConfig",
    "payload": {
        "config": "{\"$schema\":\"http://adaptivecards.io/schemas/adaptive-card.json\",\"type\":\"AdaptiveCard\",\"version\":\"1.5\",\"body\":[{\"type\":\"TextBlock\",\"text\":\"Form Title\",\"weight\":\"Bolder\"}],\"actions\":[{\"type\":\"Action.Submit\",\"title\":\"Submit\"}]}",
        "name": "My HITL Form",
        "slug": "my-hitl-form",
        "preMessage": "Please fill out the form below.",
        "postMessage": "Thanks for submitting."
    }
}'
```

### Update HITL

``` bash
curl --location 'https://api-dev-ai.vithiit.com/update-record' \
--header 'x-api-key: 5ba266fb7bee9a60' \
--header 'Content-Type: application/json' \
--data '{
    "collectionName": "hitlConfig",
    "recordId": "68a82bd74263a04967b0643e",
    "document": {
        "config": "{\"$schema\":\"http://adaptivecards.io/schemas/adaptive-card.json\",\"type\":\"AdaptiveCard\",\"version\":\"1.5\",\"body\":[{\"type\":\"TextBlock\",\"text\":\"Form Title\",\"weight\":\"Bolder\"}],\"actions\":[{\"type\":\"Action.Submit\",\"title\":\"Submit\"}]}",
        "name": "My HITL Form Updated",
        "slug": "my-hitl-form",
        "preMessage": "Please fill out the form below.",
        "postMessage": "Thanks for submitting."
    }
}'
```