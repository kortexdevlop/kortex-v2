## Overview
Kortex Mind is Kortex's assistant. You describe what you want to do; Mind picks among the Kortex tools, proposes a readable plan, and runs it on your device once you approve it. It relies on the Gemma model, hosted by Cloudflare Workers AI.
## Usage
Sign in, write your request (quoted texts are kept on your device), then approve the proposed plan. Mind runs the tools one by one, shows the results and can chain the next step. Your AI credits are shown as a percentage and refill every three hours. An executed plan can be exported as a workflow for Kortex Flow.
## Frequently asked questions
### Are my files sent to the AI?
No. The model only receives the name, type and size of your attachments, never their content, except for a text you explicitly choose to share. Tools run locally.
### Do I have to confirm every action?
Yes: nothing runs without your approval of the plan, and sensitive, network or disk-writing actions need one more confirmation.
### Why can the AI be paused?
Workers AI's free quota is shared by all users and capped each day. When it is reached, Mind says so and shows when it resumes.
