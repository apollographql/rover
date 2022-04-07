const formatHostName = (hostname) =>
  hostname.replace(/^\.*/, ".").toLowerCase();

const parseNoProxyZone = (zone) => {
  zone = zone.trim();

  const zoneParts = zone.split(":", 2);
  const zoneHost = formatHostName(zoneParts[0]);
  const zonePort = zoneParts[1];
  const hasPort = zone.indexOf(":") > -1;

  return { hostname: zoneHost, port: zonePort, hasPort: hasPort };
};

const urlInNoProxy = (requestURL, noProxy) => {
  const port =
    requestURL.port || (requestURL.protocol === "https:" ? "443" : "80");

  // clean hostname
  const hostname = formatHostName(requestURL.hostname);

  // convert to array
  const noProxyList = noProxy.split(",");

  // iterate over noProxyList and find match with RequestURL
  return noProxyList.map(parseNoProxyZone).some((noProxyZone) => {
    const isMatchedAt = hostname.indexOf(noProxyZone.hostname);
    const hostnameMatched =
      isMatchedAt > -1 &&
      isMatchedAt === hostname.length - noProxyZone.hostname.length;

    if (noProxyZone.hasPort) {
      return port === noProxyZone.port && hostnameMatched;
    }

    return hostnameMatched;
  });
};

const getProxyEnv = (requestURL) => {
  const noProxy = process.env.NO_PROXY || process.env.no_proxy || "";

  // if the noProxy is a wildcard then return null
  if (noProxy === "*") {
    return null;
  }

  // if the noProxy is not empty and the uri is found, return null
  if (noProxy !== "" && urlInNoProxy(requestURL, noProxy)) {
    return null;
  }

  // get proxy based on request url's protocol
  if (requestURL.protocol == "http:") {
    return process.env.HTTP_PROXY || process.env.http_proxy || null;
  }

  if (requestURL.protocol == "https:") {
    return process.env.HTTPS_PROXY || process.env.https_proxy || null;
  }

  // not a supported protocol...
  return null;
};

const configureProxy = (requestURL) => {
  const url = new URL(requestURL);
  const env = getProxyEnv(url);

  // short circuit if null
  if (!env) return null;

  // parse proxy url
  const { hostname, port, protocol, username, password } = new URL(env);

  // return proxy object for axios request
  return {
    proxy: {
      protocol,
      hostname,
      port,
      auth: {
        username,
        password,
      },
    },
  };
};

module.exports = {
  configureProxy,
};
