export const metadata = {
  title: "ENGRAVE V2 (Phase A placeholder)",
  description:
    "Build-verifying placeholder — no real UI yet. See V2/docs/roadmap/14-web-application.md.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
