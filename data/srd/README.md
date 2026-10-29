# The SRD 5.2.1 in the data format

dnt has these rules built in: the System Reference Document 5.2.1, in the
data format, made by [`scripts/srd`](../../scripts/srd) from the SRD's PDF:

```sh
python3 scripts/srd/convert.py SRD_CC_v5.2.1.pdf data/srd
```

Do not edit the files here by hand: change the converter and run it again.

## Attribution

This work includes material from the System Reference Document 5.2.1
("SRD 5.2.1") by Wizards of the Coast LLC, available at
https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the
Creative Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.

The material here is changed from the SRD 5.2.1: its text is set in the
data format (tagged, split into fields and tables).
