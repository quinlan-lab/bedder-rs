<<DOWNLOAD
wget -q -O - https://hgdownload.soe.ucsc.edu/goldenPath/hg38/database/simpleRepeat.txt.gz \
	| zcat | cut -f 2-5,17 | grep -v _alt | grep -v _random | grep -v _fix \
	| bedtools sort -g hg38.fai \
	| bgzip -c > simple-repeats.bed.gz

wget https://ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab/release/AshkenazimTrio/HG002_NA24385_son/mosaic_v1.00/GRCh38/SNV/HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz
wget https://ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab/release/AshkenazimTrio/HG002_NA24385_son/mosaic_v1.00/GRCh38/SNV/HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz.tbi
DOWNLOAD

vcf=HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz

#<<FULL
time ./target/release/bedder intersect -a $vcf -b simple-repeats.bed.gz -g hg38.fai | grep -cv ^#
time bedtools intersect -a $vcf -b simple-repeats.bed.gz -g hg38.fai | grep -cv ^#


time ./target/release/bedder intersect -a $vcf -b simple-repeats.bed.gz -g hg38.fai \
	--python manuscript/repeat-sequence.py  -c py:repeat_sequence -o vcf.repeats.bcf
#FULL

region=chr19
bcftools view -r $region HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.vcf.gz -O z -o HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.$region.vcf.gz
sub_vcf=HG002_GRCh38_MosaicSNVv1.0_GermlineV4.2.1.chr19.vcf.gz

time ./target/release/bedder intersect -a $sub_vcf -b simple-repeats.bed.gz -g hg38.fai | grep -cv ^#
time bedtools intersect -a $sub_vcf -b simple-repeats.bed.gz -g hg38.fai | grep -cv ^#
