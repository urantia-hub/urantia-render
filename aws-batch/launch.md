# AWS Spot Batch — Launch Guide

Based on the research report in `docs/aws-batch-research-prompt.md` + its findings:
**c7a.16xlarge spot in us-east-2, concurrency 12, S3 + CloudFront egress → ~$12, ~10hr.**

## Prereqs (one-time)

1. **AWS account + CLI configured.** `aws configure` with an IAM user that can launch EC2, manage S3, read EC2 metadata. Region: `us-east-2`.
2. **Key pair** for SSH: `aws ec2 create-key-pair --key-name urantia-batch --query KeyMaterial --output text --region us-east-2 > ~/.ssh/urantia-batch.pem && chmod 400 ~/.ssh/urantia-batch.pem`
3. **S3 bucket.** Single bucket in `us-east-2` to collect outputs:
   ```bash
   aws s3 mb s3://urantia-render-batch --region us-east-2
   ```
4. **IAM instance profile** with `s3:PutObject` + `s3:ListBucket` on the bucket, plus `ec2:TerminateInstances` if you want auto-shutdown. Quick script at the bottom of this doc.
5. **Security group** allowing inbound SSH from your IP and outbound 443:
   ```bash
   aws ec2 create-security-group --group-name urantia-batch-sg --description "urantia render batch" --region us-east-2
   aws ec2 authorize-security-group-ingress --group-name urantia-batch-sg --protocol tcp --port 22 --cidr $(curl -s ifconfig.me)/32 --region us-east-2
   ```

## Launch a spot instance

```bash
export BUCKET=urantia-render-batch
export KEY=urantia-batch

# Prepare user-data with the bucket substituted in.
sed "s|\$S3_BUCKET|$BUCKET|g" aws-batch/user-data.sh > /tmp/user-data.sh
# Or pass via env instead — simpler: edit user-data.sh top to hardcode S3_BUCKET.

aws ec2 run-instances \
  --region us-east-2 \
  --instance-type c7a.16xlarge \
  --image-id $(aws ec2 describe-images --region us-east-2 \
      --owners amazon --filters \
      "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*" \
      --query "sort_by(Images, &CreationDate)[-1].ImageId" --output text) \
  --key-name $KEY \
  --security-groups urantia-batch-sg \
  --instance-market-options 'MarketType=spot,SpotOptions={InstanceInterruptionBehavior=terminate}' \
  --iam-instance-profile Name=urantia-batch-role \
  --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=200,VolumeType=gp3}' \
  --user-data file://aws-batch/user-data.sh \
  --tag-specifications 'ResourceType=instance,Tags=[{Key=Name,Value=urantia-render-batch}]'
```

Grab the instance ID from the output, then:

```bash
IID=i-0abcdef…
PUBLIC_IP=$(aws ec2 describe-instances --region us-east-2 --instance-ids $IID \
    --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

# Tail the bootstrap log remotely
ssh -i ~/.ssh/$KEY.pem ubuntu@$PUBLIC_IP tail -f /var/log/urantia-render-batch.log
```

## Cost ballpark (validated)

| Item | Expected |
|---|---|
| c7a.16xlarge spot × 10hr @ ~$1.45 | ~$14.50 |
| EBS gp3 200 GB × 10hr | ~$0.13 |
| S3 storage 150 GB for a few days | ~$0.50 |
| Egress (via CloudFront 1TB free tier) | $0 |
| **Total** | **~$15** |

If you'd rather squeeze every dollar, switch to GPU path: `g6.12xlarge`, set `URANTIA_RENDER_ENCODER=nvenc`, plus a slightly different bootstrap to install CUDA drivers. Expect ~3hr wall-clock for ~$6 total — but another ~2hr of setup work the first time.

## Recommended pre-flight (strongly): benchmark 3 papers first

Before the full batch, launch with `PAPER_RANGE=0,1,2` to confirm per-paper timing on actual content. Target: ~30-40 min per paper at concurrency 12. If it's consistently under 30 min, libx264 is faster than modeled and you can safely bump concurrency to 16.

## Bring the videos home

After the batch completes, the instance self-terminates (if `AUTO_TERMINATE=1`). Pull the MP4s from S3 to your laptop:

```bash
aws s3 sync s3://$BUCKET/urantia-render/$(date -u +%Y-%m-%d)/videos/ ./output/videos/
```

Or front the bucket with a CloudFront distribution for free 1TB egress.

## Safety nets

- `--skip-existing` means an interrupted spot resumes cleanly on relaunch.
- `aws s3 sync` at the end is idempotent — rerunning uploads only delta files.
- If bootstrap fails (build error, network hiccup), SSH in, fix, re-run just the relevant steps — the `set -euxo pipefail` trace in `/var/log/urantia-render-batch.log` makes it obvious where it stopped.

## Creating the IAM instance profile (one-time)

```bash
# Trust policy (assume-role for EC2)
cat > /tmp/trust.json <<'EOF'
{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}
EOF

aws iam create-role --role-name urantia-batch-role --assume-role-policy-document file:///tmp/trust.json

cat > /tmp/policy.json <<EOF
{"Version":"2012-10-17","Statement":[
  {"Effect":"Allow","Action":["s3:PutObject","s3:PutObjectAcl","s3:ListBucket","s3:GetObject"],"Resource":["arn:aws:s3:::$BUCKET","arn:aws:s3:::$BUCKET/*"]},
  {"Effect":"Allow","Action":"ec2:TerminateInstances","Resource":"*"}
]}
EOF

aws iam put-role-policy --role-name urantia-batch-role --policy-name batch --policy-document file:///tmp/policy.json
aws iam create-instance-profile --instance-profile-name urantia-batch-role
aws iam add-role-to-instance-profile --instance-profile-name urantia-batch-role --role-name urantia-batch-role
```
