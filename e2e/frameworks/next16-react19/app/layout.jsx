export default function Layout({ children }) {
  return (
    <html>
      <head>
        <link rel="icon" href="data:," />
      </head>
      <body>{children}</body>
    </html>
  );
}
