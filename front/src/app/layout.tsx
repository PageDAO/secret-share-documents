import "./globals.css";

import { headers } from "next/headers";
import { cookieToInitialState } from "wagmi";
import { config } from "@/config";
import { ContextProvider } from "@/context";
import { ReactNode } from "react";
import { Toaster } from "@/components/ui/toaster"

export default function RootLayout({
  children,
}: Readonly<{
  children: ReactNode;
}>) {
  const initialState = cookieToInitialState(config, headers().get("cookie"));
  return (
    <html lang="en">
      <body className="flex flex-col min-h-screen">
        <ContextProvider initialState={initialState}>
            {children}
            <Toaster />
        </ContextProvider>
      </body>
    </html>
  );
}
