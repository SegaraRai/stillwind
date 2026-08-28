module.exports = function ({ addUtilities }) {
  addUtilities({
    ".legacy-skew": {
      transform: "skewY(-10deg)",
    },
  });
};
