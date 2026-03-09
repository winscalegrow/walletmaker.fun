import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "WalletMaker | Secure Solana Vanity Generator",
  description: "Securely generate your custom Solana vanity addresses seamlessly in the browser.",
  icons: {
    icon: "/logo.svg",
  },
  openGraph: {
    title: "WalletMaker",
    description: "Lightning fast, 100% secure client-side Solana vanity address generator.",
    images: ["/logo.svg"],
  }
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        {children}
      </body>
    </html>
  );
}
