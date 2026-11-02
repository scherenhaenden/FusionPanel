import re

with open("integration/containers/src/routes.rs", "r") as f:
    content = f.read()

content = content.replace('"/v1/containers/:id"', '"/v1/containers/{id}"')
content = content.replace('"/v1/containers/:id/start"', '"/v1/containers/{id}/start"')
content = content.replace('"/v1/containers/:id/stop"', '"/v1/containers/{id}/stop"')
content = content.replace('"/v1/containers/:id/restart"', '"/v1/containers/{id}/restart"')

with open("integration/containers/src/routes.rs", "w") as f:
    f.write(content)
