# winhelper

A tiny native Windows 10/11 panel showing CPU / RAM / GPU load, public IP,
LAN IP and the approximate location of the public IP.

## Privacy

To show the public IP and its location, WinHelper sends one HTTPS request to a
third-party service when it starts and whenever you choose **Refresh IP now**.
That service sees your public IP address. It tries these in order and stops at
the first one that answers:

1. [ipwho.is](https://ipwho.is/)
2. [ipinfo.io](https://ipinfo.io/)
3. [ipify](https://www.ipify.org/) (IP only, no location)

Nothing else is sent; the request carries only the generic user agent
`WinHelper/0.1`. The LAN IP is read locally and never leaves the machine.
A setting to turn the lookup off is planned (milestone M4).
