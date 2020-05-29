import React from 'react';
import { RelayProp, createFragmentContainer } from 'react-relay';
import graphql from 'babel-plugin-relay/macro';
import { HardwareSpecListItem_hardwareSpec } from './__generated__/HardwareSpecListItem_hardwareSpec.graphql';
import { List, ListItem, ListItemText, makeStyles } from '@material-ui/core';
import { Link as RouterLink } from 'react-router-dom';

const useLocalStyles = makeStyles(({ spacing }) => ({
  nestedListItem: {
    paddingLeft: spacing(4),
  },
}));

const HardwareSpecListItem: React.FC<{
  hardwareSpec: HardwareSpecListItem_hardwareSpec;
  relay: RelayProp;
}> = ({ hardwareSpec }) => {
  const localClasses = useLocalStyles();

  return (
    <>
      <ListItem
        key={hardwareSpec.id}
        button
        component={RouterLink}
        to={`/hardware/${hardwareSpec.slug}`}
      >
        <ListItemText primary={hardwareSpec.slug} />
      </ListItem>
      <List dense disablePadding>
        {hardwareSpec.programSpecs.edges.map(({ node: programSpec }) => (
          <ListItem
            key={programSpec.id}
            className={localClasses.nestedListItem}
            button
            component={RouterLink}
            to={`/hardware/${hardwareSpec.slug}/puzzles/${programSpec.slug}`}
          >
            <ListItemText
              primary={`${hardwareSpec.slug}/${programSpec.slug}`}
              secondary={
                programSpec.userPrograms.totalCount > 0
                  ? `${programSpec.userPrograms.totalCount} solutions`
                  : undefined
              }
            />
          </ListItem>
        ))}
      </List>
    </>
  );
};

export default createFragmentContainer(HardwareSpecListItem, {
  hardwareSpec: graphql`
    fragment HardwareSpecListItem_hardwareSpec on HardwareSpecNode {
      id
      slug
      ...HardwareSpecSummary_hardwareSpec
      programSpecs(first: 5) {
        totalCount
        edges {
          node {
            id
            slug
            userPrograms {
              totalCount
            }
          }
        }
      }
    }
  `,
});
