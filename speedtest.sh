#!/bin/sh
if [ ! -f JSON.sh ]; then
    curl https://raw.githubusercontent.com/dominictarr/JSON.sh/master/JSON.sh -o JSON.sh
fi

curl "https://www.speedtest.net/api/js/servers?engine=js" | sh JSON.sh -l | while read -r key value; do
    case "$key" in
    *sponsor*)
        # Remove ""
        sponsor="${value#\"}"
        sponsor="${sponsor%\"}"
        ;;
    *host*)
        host="${value#\"}"
        host="${host%\"}"
        printf "$sponsor:  $host\n"
        ;;
    esac
done
