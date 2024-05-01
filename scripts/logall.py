import subprocess
import sys
import re

# Command to get the list of deployments
deployment_cmd = "cargo shuttle deployment list --raw"

# Run the command and capture the output
output = subprocess.check_output(deployment_cmd, shell=True, text=True)

# Split the output into lines
lines = output.strip().split("\n")

# Extract the UUIDs from the lines
uuids = re.findall(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", output
)

# Get logs for each UUID and concatenate them
logs = ""
for uuid in uuids:
    log_cmd = f"cargo shuttle logs {uuid} --all"
    log_output = subprocess.check_output(log_cmd, shell=True, text=True)
    logs += log_output

# Write the logs to a file or print to stdout
if len(sys.argv) > 1:
    output_file = sys.argv[1]
    with open(output_file, "w") as file:
        file.write(logs)
    print(f"Logs written to {output_file}")
else:
    print(logs)
