FROM debian:bookworm-slim

# python3 is used by the subject config rewrite and synthetic-suite oracles.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates python3 \
    && rm -rf /var/lib/apt/lists/*

COPY target/debug/first-principles-agent /usr/local/bin/1agent
COPY harness/1ash/subject_1agent.sh /opt/1ash/subject_1agent.sh
COPY harness/1ash/tests/fake_subject.sh /opt/1ash/fake_subject.sh
COPY harness/1ash/tests/noop_subject.sh /opt/1ash/noop_subject.sh

RUN chmod 0755 /usr/local/bin/1agent /opt/1ash/subject_1agent.sh \
    /opt/1ash/fake_subject.sh /opt/1ash/noop_subject.sh

WORKDIR /workspace
