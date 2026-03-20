FROM ubuntu:24.04

RUN apt-get update && apt-get install -y curl unzip && rm -rf /var/lib/apt/lists/*

# Copy install script and test runner
COPY install.sh /tmp/install.sh
COPY e2e/run.sh /tmp/run.sh

# Copy sample JS file for checking
COPY e2e/sample.js /test/sample.js

RUN chmod +x /tmp/run.sh

CMD ["/tmp/run.sh"]
