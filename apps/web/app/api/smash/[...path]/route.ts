import { NextRequest, NextResponse } from "next/server";

const BACKEND_BASE_URL = process.env.SMASH_API_BASE_URL ?? "http://127.0.0.1:3000";

function targetUrl(path: string[], request: NextRequest) {
  const pathname = path.join("/");
  const url = new URL(`/api/${pathname}`, BACKEND_BASE_URL);
  request.nextUrl.searchParams.forEach((value, key) => {
    url.searchParams.set(key, value);
  });
  return url;
}

async function proxy(request: NextRequest, path: string[]) {
  const url = targetUrl(path, request);
  const headers = new Headers();
  const contentType = request.headers.get("content-type");
  if (contentType) {
    headers.set("content-type", contentType);
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    headers.set("X-Smash-Local-Action", "true");
  }

  const response = await fetch(url, {
    method: request.method,
    headers,
    body: request.method === "GET" || request.method === "HEAD" ? undefined : await request.text(),
    cache: "no-store"
  });

  const body = await response.text();
  return new NextResponse(body, {
    status: response.status,
    headers: {
      "content-type": response.headers.get("content-type") ?? "application/json"
    }
  });
}

export async function GET(request: NextRequest, context: { params: Promise<{ path: string[] }> }) {
  const { path } = await context.params;
  return proxy(request, path);
}

export async function POST(request: NextRequest, context: { params: Promise<{ path: string[] }> }) {
  const { path } = await context.params;
  return proxy(request, path);
}
