#!/bin/env python3
# -*- coding: utf-8 -*-
import requests

r = requests.get('https://www.speedtest.net/api/js/servers?engine=js')
r.raise_for_status()
for s in r.json():
    print('{}: {}'.format(s['sponsor'],s['host']))
