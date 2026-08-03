export const metadata = {
  title: "ENGRAVE V2",
  description: "Governed Memory review and source evidence.",
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
