# Nettest

A command line tool to measure network delay and bandwidth.

```output
nettest
All-in-one network test tool

USAGE:
    nettest [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --count <count>          Count of times to test
    -i, --interval <interval>    Wait interval ms between echo test [default: 1000]
    -s, --size <size>            Length of test payload. The unit is byte in ping test and Megabyte in bandwidth test
                                 [default: 60]
    -t, --timeout <timeout>      Timeout of each test (in seconds) [default: 5]

SUBCOMMANDS:
    help            Prints this message or the help of the given subcommand(s)
    ping            Measuring latency using ICMP or ICMPv6 echo" example: `nettest ping 127.0.0.1` or `nettest ping
                    google.com`
    quicdownload    Measuring QUIC download bandwidth
    quicupload      Measuring QUIC upload bandwidth
    tcpdownload     Measuring TCP download bandwidth
    tcping          Measuring latency of TCP shake hands example: `nettest tcping 127.0.0.1:8080` or `nettest ping
                    github.com:443`
    tcpupload       Measuring TCP upload bandwidth
    udping          Measuring latency using UDP echo. use `socat -v UDP-LISTEN:8000,fork PIPE` to start a server"
                    example: `nettest udping 127.0.0.1:8000`
```