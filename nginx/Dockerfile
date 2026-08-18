FROM nginx:latest

# Remove the default configuration file
RUN rm /etc/nginx/conf.d/default.conf

# Copy your custom nginx configuration file
COPY nginx.conf /etc/nginx/nginx.conf

# Copy the SSL certificates (optional: if you're using self-signed certs)
COPY ssl/cert.pem /etc/nginx/ssl/cert.pem
COPY ssl/key.pem /etc/nginx/ssl/key.pem

