import "./globals.css";
import type { Metadata } from "next";
import { headers } from "next/headers";

import { cookieToInitialState } from "wagmi";

import { config } from "@/config";
import { ContextProvider } from "@/context";

import { ThirdwebProvider, ConnectButton } from "@/app/thirdweb";
import { createThirdwebClient } from "thirdweb";
 
const client = createThirdwebClient({
  clientId: "_-K9PstFJcBUK4eKcIIc5XVzSTulpQjqt74AijygyFzme7FTShFBU6SI6JaPOjoDS6R3h_aAzuhfyA3eDUyeEA",
});

export const metadata: Metadata = {
  title: "PageDAO Secret Documents",
  description: "Secured by Secret, powered by PageDAO and FiftyWei",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const initialState = cookieToInitialState(config, headers().get("cookie"));
  return (
    <html lang="en">
      <body>
        <ThirdwebProvider client={client}>
          <ContextProvider initialState={initialState}>
            {children}
          </ContextProvider>
        </ThirdwebProvider>
      </body>
    </html>
  );
}
