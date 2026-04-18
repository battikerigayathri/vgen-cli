const _ = require('lodash');

async function handler(event) {
  const context = event.context;
  return {
    context,
    message: 'Sample Tool Working!'
  };
}

module.exports = { handler };