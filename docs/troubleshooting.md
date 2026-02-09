# Troubleshooting

## Authentication failures

ssh-hub authenticates using SSH keys only (no passwords). When `ssh-hub add` fails with "failed authentication", the server is reachable but rejected all offered keys. The remote server must have your **public key** authorized before ssh-hub can connect.

### Standard servers (authorized_keys)

Copy your public key to the server using an existing access method (e.g., console, another SSH session):

```bash
# From a machine that already has access:
ssh-copy-id -i ~/.ssh/my_key.pub user@host

# Or manually append to ~/.ssh/authorized_keys on the server:
echo "ssh-rsa AAAA..." >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys
```

### Cloud providers

Cloud VMs don't use `authorized_keys` by default — keys are managed through the provider's metadata service. When you replace a boot disk or create a fresh instance, SSH keys must be re-provisioned through the cloud CLI.

#### GCP

```bash
# Add your public key to the instance metadata, bound to a username
gcloud compute instances add-metadata INSTANCE --zone=ZONE \
  --metadata=ssh-keys="USERNAME:$(cat ~/.ssh/my_key.pub)"

# Add to ssh-hub
ssh-hub add my-server USERNAME@host
```

If the instance has OS Login enabled, disable it first so metadata keys take effect:

```bash
gcloud compute instances remove-metadata INSTANCE --zone=ZONE --keys=enable-oslogin
```

#### AWS

```bash
# Connect with the original key pair assigned at launch, then add your key
ssh -i original_key.pem ec2-user@host \
  "echo '$(cat ~/.ssh/my_key.pub)' >> ~/.ssh/authorized_keys"
```

Alternatively, use EC2 Instance Connect to push a temporary key:

```bash
aws ec2-instance-connect send-ssh-public-key \
  --instance-id i-0123456789abcdef0 \
  --instance-os-user USERNAME \
  --ssh-public-key file://~/.ssh/my_key.pub
```

### Passphrase-protected keys

ssh-hub cannot decrypt passphrase-protected keys at runtime. Use the `-i` flag during `add` to load the key into your SSH agent:

```bash
ssh-hub add my-server user@host -i ~/.ssh/my_key
```

This runs `ssh-add` which prompts for the passphrase once and stores the decrypted key in the agent. ssh-hub then authenticates through the agent.

If the key is already in your agent (`ssh-add -l` to check), ssh-hub will find it automatically — no `-i` needed.
