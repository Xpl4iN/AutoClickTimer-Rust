# AutoClick Remote

The Android companion app controls AutoClick Timer over Tailscale.

1. Connect the PC and phone to the same Tailscale network.
2. Open **Pair phone** in the Windows AutoClick Timer app. Use **Download phone app** there if needed.
3. In AutoClick Remote, tap **Scan pairing code** and scan the QR code shown on the PC. Alternatively, enter the displayed host, port, and pairing key manually.
4. The connection details are saved on the phone. The app reconnects automatically on later launches. Use **Disconnect from Host** to stop automatic reconnection.

The Windows app must be running for the phone to connect. If Tailscale starts after the Windows app, restart AutoClick Timer to start its Tailscale listener.
