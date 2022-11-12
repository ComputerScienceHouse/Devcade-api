FROM node:18-bullseye

# Default node app location
WORKDIR /usr/src/app

# Copy and install node dependencies
COPY package*.json ./
RUN npm install

# Change cache settings
RUN mkdir ./my_cache; npm config set cache ./my_cache --global; npm --global cache verify

# Create downloads directory
RUN mkdir ./downloads ./uploads

# Copy the rest of the app
COPY . .

# Permissions
RUN chmod -R 775 /usr/src/app && chown -R node:node /usr/src/app

# Entrypoint
USER 1000
EXPOSE 8080
CMD ["npm", "start"]

