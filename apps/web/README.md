# Smash Web

Parallel Next.js frontend for phase 3 of the roadmap.

## Run

```bash
cd apps/web
npm install
npm run dev
```

Set `SMASH_API_BASE_URL` if the local Smash API is not running on `http://127.0.0.1:3000`.

The app proxies browser requests through `/api/smash/*` so mutation routes can call the local backend without CORS issues.
