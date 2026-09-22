<character>
  <name>${name}</name>
  <race>${race}</race>
  <level>${level}</level>
  <hp>${hp}</hp>
  <ac>${ac}</ac>
  <bab>${bab}</bab>
  <#if hp gt 0><alive>true</alive></#if>
  <skills>
<#list skills as skill>    <skill name="${skill.name}" value="${skill.value}"/>
</#list>  </skills>
  <spells>
<#list spells as spell>    <spell name="${spell.name}" dc="${spell.dc}"/>
</#list>  </spells>
</character>
